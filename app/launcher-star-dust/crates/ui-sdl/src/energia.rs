//! Leitura de energia: bateria, carga e temperatura.
//!
//! ## Por que isto existe
//!
//! O `giochi-selftest` do sistema mostra, nos dois aparelhos:
//!
//! ```text
//! [PASS] bateria: 0% (Not charging)
//! [PASS] temperatura: 42C
//! ```
//!
//! E a HUD do launcher mostrava `—` nos dois. O dado existe, é publicado pelo
//! kernel, e não era lido — não porque fosse difícil, mas porque nunca ninguém
//! tinha ligado as duas pontas. Contrato em `docs/integracao.md` §2.3 e §2.4.
//!
//! ## Separar "sem leitura" de "leitura 0%"
//!
//! **O X55 não tem bateria.** Ela foi removida fisicamente para dar acesso à
//! serial, e o PMIC continua publicando `rk817-battery` com `0% / Not charging`
//! porque o nó do sysfs existe — não porque haja célula atrás dele. Foi uma
//! conclusão minha errada: cheguei a registrar que era "leitura válida com valor
//! ruim". É leitura de coisa que não está lá.
//!
//! Consequência prática: **este módulo não pode ser julgado no X55.** Quem
//! valida bateria é o outro aparelho, que tem célula. O que dá para conferir aqui
//! é temperatura, que é sensor do SoC e independe da bateria.
//!
//! Ainda assim o modelo separa os casos, porque o dia em que houver bateria a
//! distinção importa:
//!
//! | Bateria | Significado | Tela |
//! | --- | --- | --- |
//! | `None` | nenhum `power_supply` do tipo `Battery` | `—` |
//! | `Some(0.0)` | mediu, e está em zero | `0%` |
//! | `Some(42.0)` | mediu, e está em 42 | `42%` |
//!
//! E `carregando` viaja separado, porque `0%` no cabo e `0%` fora dele são
//! situações diferentes.
//!
//! ## Por que não escolher o dispositivo pelo nome
//!
//! O gauge aparece como `rk817-battery` neste PMIC, `BAT0` em outro aparelho e
//! `battery` num terceiro. Filtrar por `type` é o que funciona nos três — a
//! mesma decisão que já está registrada em `docs/integracao.md` §2.3.

use std::path::{Path, PathBuf};

/// Leitura de bateria num instante.
#[derive(Clone, Debug, PartialEq)]
pub struct Bateria {
    /// Percentual 0..=100. `None` = não há medidor; a UI mostra `—`.
    pub percentual: Option<f32>,
    /// Ligado ao carregador. Vem de `status`.
    pub carregando: bool,
    /// Texto cru de `status`, para o log. `Charging`, `Discharging`,
    /// `Not charging`, `Full` — vocabulário do driver, não nosso.
    pub status: String,
}

impl Bateria {
    /// Interpreta `capacity` + `status`. Separado para testar sem `/sys`.
    pub fn de_valores(capacity: Option<&str>, status: Option<&str>) -> Self {
        let status = status.unwrap_or("").trim().to_string();
        // `Charging` é o único que significa "puxando energia". `Full` e
        // `Not charging` com o cabo ligado NÃO são carga — e tratar os dois
        // como "carregando" faria a UI dizer que está carregando num aparelho
        // que só está segurado pelo cabo.
        let carregando = status.eq_ignore_ascii_case("Charging");

        let percentual = capacity
            .and_then(|v| v.trim().parse::<f32>().ok())
            // Fora de 0..=100 é driver entregando lixo (já houve `-1` em gauge
            // não calibrado). Vira "sem leitura" em vez de virar número na tela.
            .filter(|v| (0.0..=100.0).contains(v));

        Self {
            percentual,
            carregando,
            status,
        }
    }
}

/// Fontes de energia descobertas nesta máquina.
pub struct Energia {
    /// Pasta do `power_supply` do tipo `Battery`, se houver.
    bateria: Option<PathBuf>,
    /// `thermal_zone*` na ordem em que serão lidas, com o `type` de cada uma.
    zonas: Vec<(PathBuf, String)>,
}

impl Energia {
    /// Descobre as fontes em `/sys`. Ausência não é erro: no PC não existe nada
    /// disso e o launcher precisa continuar funcionando.
    pub fn discover() -> Self {
        Self::de_raizes(
            Path::new("/sys/class/power_supply"),
            Path::new("/sys/class/thermal"),
        )
    }

    /// Mesma descoberta, com raízes injetáveis — para testar sem hardware.
    pub fn de_raizes(power: &Path, thermal: &Path) -> Self {
        let mut bateria = None;
        if let Ok(entradas) = std::fs::read_dir(power) {
            let mut candidatos: Vec<PathBuf> = entradas.flatten().map(|e| e.path()).collect();
            // Ordem estável: dois medidores não podem alternar a cada quadro e
            // fazer o número piscar entre dois valores.
            candidatos.sort();
            for pasta in candidatos {
                let tipo = ler_texto(&pasta.join("type")).unwrap_or_default();
                if tipo.eq_ignore_ascii_case("Battery") {
                    bateria = Some(pasta);
                    break;
                }
            }
        }

        let mut zonas = Vec::new();
        if let Ok(entradas) = std::fs::read_dir(thermal) {
            let mut candidatos: Vec<PathBuf> = entradas
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .map(|n| n.to_string_lossy().starts_with("thermal_zone"))
                        .unwrap_or(false)
                })
                .collect();
            candidatos.sort();
            for pasta in candidatos {
                let tipo = ler_texto(&pasta.join("type")).unwrap_or_else(|| "?".to_string());
                zonas.push((pasta, tipo));
            }
        }

        Self { bateria, zonas }
    }

    /// Lê bateria. `None` só quando não há medidor — ler e falhar devolve
    /// `Some` com percentual `None`, que é informação diferente.
    pub fn bateria(&self) -> Option<Bateria> {
        let pasta = self.bateria.as_ref()?;
        let capacidade = ler_texto(&pasta.join("capacity"));
        let status = ler_texto(&pasta.join("status"));
        Some(Bateria::de_valores(capacidade.as_deref(), status.as_deref()))
    }

    /// Temperatura do SoC em °C inteiros.
    ///
    /// Prefere a zona de `cpu`/`soc`, que é o que esquenta e o que o usuário
    /// quer ver; `gpu` e as demais são plano B. `temp` vem em **miligraus**.
    pub fn temperatura(&self) -> Option<u16> {
        self.temperatura_de("cpu")
    }

    /// Mesma leitura, escolhendo a zona por substring do `type`.
    pub fn temperatura_de(&self, prefere: &str) -> Option<u16> {
        let escolhida = self
            .zonas
            .iter()
            .find(|(_, tipo)| tipo.to_ascii_lowercase().contains(prefere))
            .or_else(|| self.zonas.first())?;
        let texto = ler_texto(&escolhida.0.join("temp"))?;
        let mili: i64 = texto.trim().parse().ok()?;
        // Miligraus → graus. Negativo seria sensor invertido; clampar em 0 é
        // melhor que mostrar temperatura negativa num SoC ligado.
        Some((mili / 1000).clamp(0, 150) as u16)
    }
}

/// Lê um arquivo de sysfs e devolve o conteúdo sem espaços nas pontas.
fn ler_texto(caminho: &Path) -> Option<String> {
    std::fs::read_to_string(caminho)
        .ok()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_percentual_e_carga() {
        let b = Bateria::de_valores(Some("42\n"), Some("Charging\n"));
        assert_eq!(b.percentual, Some(42.0));
        assert!(b.carregando);
    }

    /// O caso medido no X55: `0% Not charging` com o aparelho no cabo.
    /// A leitura é válida (não é ausência) e o valor é zero — a UI precisa
    /// poder dizer os dois.
    #[test]
    fn zero_por_cento_nao_e_ausencia() {
        let b = Bateria::de_valores(Some("0"), Some("Not charging"));
        assert_eq!(b.percentual, Some(0.0), "0% é leitura, não ausência");
        assert!(!b.carregando, "Not charging não é carga");
    }

    /// `Full` com o cabo ligado não é `Charging`. Tratar como carga faria a UI
    /// mentir num aparelho que já está cheio.
    #[test]
    fn full_nao_e_carregando() {
        assert!(!Bateria::de_valores(Some("100"), Some("Full")).carregando);
    }

    #[test]
    fn sem_capacity_vira_ausencia() {
        let b = Bateria::de_valores(None, Some("Discharging"));
        assert_eq!(b.percentual, None);
    }

    #[test]
    fn valor_fora_de_faixa_e_descartado() {
        for ruim in ["-1", "101", "abc", ""] {
            let b = Bateria::de_valores(Some(ruim), Some("Discharging"));
            assert_eq!(b.percentual, None, "{ruim} deveria ser descartado");
        }
    }

    #[test]
    fn le_temperatura_em_miligraus() {
        let dir = tempdir();
        let zona = dir.join("thermal_zone0");
        std::fs::create_dir_all(&zona).unwrap();
        std::fs::write(zona.join("type"), "cpu-thermal\n").unwrap();
        std::fs::write(zona.join("temp"), "37777\n").unwrap();

        let energia = Energia::de_raizes(&dir.join("power"), &dir);
        assert_eq!(energia.temperatura(), Some(37));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Prefere a zona de CPU quando existe mais de uma: `gpu-thermal` é mais
    /// fria e mostraria um número que não corresponde ao que esquenta.
    #[test]
    fn prefere_zona_de_cpu() {
        let dir = tempdir();
        for (nome, tipo, temp) in [
            ("thermal_zone0", "gpu-thermal", "36000"),
            ("thermal_zone1", "cpu-thermal", "41000"),
        ] {
            let zona = dir.join(nome);
            std::fs::create_dir_all(&zona).unwrap();
            std::fs::write(zona.join("type"), tipo).unwrap();
            std::fs::write(zona.join("temp"), temp).unwrap();
        }
        let energia = Energia::de_raizes(&dir.join("power"), &dir);
        assert_eq!(energia.temperatura(), Some(41));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Sem nenhuma zona, `None` — e a HUD mostra `—` em vez de `0 °C`.
    #[test]
    fn sem_zona_termica_e_none() {
        let dir = tempdir();
        let energia = Energia::de_raizes(&dir.join("power"), &dir.join("thermal"));
        assert_eq!(energia.temperatura(), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Dispositivo de bateria é escolhido por `type`, não por nome de pasta: o
    /// gauge se chama `rk817-battery`, `BAT0` ou `battery` conforme o aparelho.
    #[test]
    fn escolhe_bateria_pelo_tipo_e_nao_pelo_nome() {
        let dir = tempdir();
        let pasta = dir.join("power_supply/nome_estranho");
        std::fs::create_dir_all(&pasta).unwrap();
        std::fs::write(pasta.join("type"), "Battery\n").unwrap();
        std::fs::write(pasta.join("capacity"), "55\n").unwrap();
        std::fs::write(pasta.join("status"), "Discharging\n").unwrap();

        let energia = Energia::de_raizes(&dir.join("power_supply"), &dir.join("thermal"));
        assert!(energia.bateria().is_some());
        assert_eq!(energia.bateria().unwrap().percentual, Some(55.0));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Só `USB`/`Mains` na pasta não é bateria: o adaptador de energia aparece
    /// como `type=USB` no X55 e não tem `capacity`.
    #[test]
    fn carregador_sozinho_nao_e_bateria() {
        let dir = tempdir();
        let pasta = dir.join("power_supply/rk817-charger");
        std::fs::create_dir_all(&pasta).unwrap();
        std::fs::write(pasta.join("type"), "USB\n").unwrap();

        let energia = Energia::de_raizes(&dir.join("power_supply"), &dir.join("thermal"));
        assert!(energia.bateria().is_none());
        assert_eq!(energia.bateria(), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    fn tempdir() -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "giochi-energia-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&base).unwrap();
        base
    }
}
