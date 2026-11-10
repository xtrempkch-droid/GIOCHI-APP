//! A arma: cooldown, raio e abate.
//!
//! ## O raio sai do centro da tela, pela frente da câmera
//!
//! O plano oferecia duas opções — inverter a matriz de visão-projeção ou usar o
//! vetor "para frente" da câmera. O vetor é o certo aqui: a mira **é** o centro
//! da tela, então o raio que parte do olho na direção da câmera passa
//! exatamente pelo pixel central. Inverter a matriz custaria uma inversão de 4×4
//! por tiro para responder a mesma pergunta.
//!
//! ## O cooldown é contado em tempo de simulação
//!
//! `atualizar(dt)` e não `Instant::now()`: assim o ritmo de tiro é o mesmo num
//! aparelho que roda a 49 fps e num que roda a 60, e o teste consegue simular
//! dez segundos em microssegundos.
//!
//! ## Três armas, um caminho de tiro
//!
//! [`ARMAS`] é uma tabela: cadência, quantos alvos por disparo, abertura do leque
//! e raio de explosão. O `PULSAR` é a arma do bloco 2a com os mesmos números —
//! um alvo, 0,15 s —, então trocar de arma não muda o comportamento da partida
//! medida, e há teste comparando o pulso com as constantes antigas.
//!
//! O leque e o morteiro **não** têm um segundo caminho de tiro: os dois passam
//! pelo mesmo [`Weapon::disparar`], que devolve a lista de cubos atingidos. Um
//! caminho por arma seria três lugares para o `FOLGA_DE_MIRA` divergir.

use crate::gameplay::collision::raycast_aabb;
use crate::gameplay::enemy_pool::EnemyPool;
use giochi_engine_3d::math3d::Vec3;

/// Intervalo mínimo entre dois tiros, em segundos.
pub const INTERVALO: f32 = 0.15;
/// Pontos por cubo abatido com a arma padrão.
///
/// Cada [`Arma`] tem o seu valor; esta constante é o do pulso e existe porque o
/// bloco 2b a usa como referência do `+100` do HUD.
pub const PONTOS_POR_ABATE: u32 = 100;

/// Uma arma: tudo o que muda entre elas, num lugar só.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arma {
    /// Identificador de linha de comando.
    pub id: &'static str,
    /// Nome desenhado na HUD.
    pub nome: &'static str,
    /// Intervalo mínimo entre dois disparos, em segundos.
    pub intervalo: f32,
    /// Quantos cubos um disparo pode abater.
    pub alvos: usize,
    /// Meia-abertura do leque, em radianos. Zero é raio exato.
    pub cone: f32,
    /// Raio da explosão em volta do impacto, em unidades. Zero não explode.
    pub explodir: f32,
    /// Pontos por cubo abatido.
    pub pontos: u32,
}

impl Arma {
    /// Uma linha dizendo o que a arma faz.
    pub fn descricao(&self) -> &'static str {
        match self.id {
            "pulsar" => "um alvo, cadencia alta",
            "leque" => "tres alvos num leque curto",
            "morteiro" => "explode e leva o que estiver perto",
            _ => "",
        }
    }
}

/// A arma do bloco 2a: um alvo, cadência alta.
pub const PULSAR: Arma = Arma {
    id: "pulsar",
    nome: "PULSAR",
    intervalo: INTERVALO,
    alvos: 1,
    cone: 0.0,
    explodir: 0.0,
    pontos: PONTOS_POR_ABATE,
};

/// Leque: três alvos por disparo, cadência três vezes menor.
///
/// A abertura é de 0,16 rad (~9°) — o suficiente para pegar dois cubos que vêm
/// lado a lado e pouco o bastante para não acertar o que está fora da mira. O
/// custo é a cadência: 0,45 s contra 0,15 s significa que um erro custa caro.
pub const LEQUE: Arma = Arma {
    id: "leque",
    nome: "LEQUE",
    intervalo: 0.45,
    alvos: 3,
    cone: 0.16,
    explodir: 0.0,
    pontos: 80,
};

/// Morteiro: um impacto que explode num raio de 3,2 unidades.
///
/// É a resposta ao pool cheio — três cubos chegando juntos morrem de uma vez — e
/// o preço é a cadência mais baixa das três (0,70 s). O raio é medido no espaço
/// do mundo, então na casca de spawn vale por vários cubos e perto do asteroide
/// por um só.
pub const MORTEIRO: Arma = Arma {
    id: "morteiro",
    nome: "MORTEIRO",
    intervalo: 0.70,
    alvos: 8,
    cone: 0.0,
    explodir: 3.2,
    pontos: 120,
};

/// As três armas, na ordem em que o jogador as percorre.
pub const ARMAS: [&Arma; 3] = [&PULSAR, &LEQUE, &MORTEIRO];

/// Índice da arma na tabela, pelo identificador.
pub fn indice_da_arma(id: &str) -> Option<usize> {
    ARMAS
        .iter()
        .position(|arma| arma.id == id.trim().to_lowercase())
}
/// Folga aplicada à caixa do inimigo, como fração do lado.
///
/// O plano pede 10 % "para sentir justo": o cubo desenhado tem 0,8 e a caixa de
/// mira tem 0,88. Sem a folga, acertar a borda de um cubo a 15 m exige precisão
/// de um pixel, e o jogador culpa a mira em vez do próprio dedo.
pub const FOLGA_DE_MIRA: f32 = 0.10;

/// O que um tiro acertou.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Atingido {
    /// Slot do cubo no pool.
    pub indice: usize,
    /// Distância do olho até a face atingida.
    pub distancia: f32,
    /// Ponto do impacto — o "+100" do bloco 2b nasce aqui.
    pub posicao: Vec3,
}

/// Cadência de tiro.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Weapon {
    /// Qual arma está na mão.
    pub arma: &'static Arma,
    /// Tempo desde o último tiro, em segundos.
    pub desde_ultimo_tiro: f32,
    /// Intervalo em vigor, já o da arma (permite ajuste sem tocar no resto).
    pub intervalo: f32,
}

impl Weapon {
    /// Arma pronta para atirar — o pulso, que é a do bloco 2a.
    pub fn new() -> Self {
        Self::com_arma(&PULSAR)
    }

    /// Arma pronta com uma arma da tabela na mão.
    pub fn com_arma(arma: &'static Arma) -> Self {
        Self {
            arma,
            // Começa "carregada": o primeiro clique tem de atirar, e um cooldown
            // inicial faria o primeiro tiro do jogador se perder.
            desde_ultimo_tiro: arma.intervalo,
            intervalo: arma.intervalo,
        }
    }

    /// Arma pronta com outra cadência e o mesmo pulso.
    pub fn com_intervalo(intervalo: f32) -> Self {
        let intervalo = if intervalo.is_finite() && intervalo > 0.0 {
            intervalo
        } else {
            INTERVALO
        };
        Self {
            arma: &PULSAR,
            // Começa "carregada": o primeiro clique tem de atirar, e um cooldown
            // inicial faria o primeiro tiro do jogador se perder.
            desde_ultimo_tiro: intervalo,
            intervalo,
        }
    }

    /// Troca a arma **mantendo o cooldown já cumprido**.
    ///
    /// Zerar o `desde_ultimo_tiro` na troca deixaria o jogador punido por ter
    /// escolhido outra arma: ele apertaria o botão e nada sairia por 0,7 s, sem
    /// nenhum sinal na tela. O que a troca muda é o **teto**, não o crédito.
    pub fn trocar(&mut self, arma: &'static Arma) {
        self.arma = arma;
        self.intervalo = arma.intervalo;
    }

    /// Troca para a arma de índice `i` na tabela, dando a volta.
    pub fn trocar_indice(&mut self, i: usize) {
        let arma = ARMAS[i % ARMAS.len()];
        self.trocar(arma);
    }

    /// Índice da arma na mão.
    pub fn indice(&self) -> usize {
        ARMAS
            .iter()
            .position(|arma| arma.id == self.arma.id)
            .unwrap_or(0)
    }

    /// Passa para a próxima arma da tabela.
    pub fn proxima(&mut self) {
        self.trocar_indice(self.indice() + 1);
    }

    /// Pontos por abate da arma na mão.
    pub fn pontos(&self) -> u32 {
        self.arma.pontos
    }

    /// Avança o relógio da arma.
    pub fn atualizar(&mut self, dt: f32) {
        if dt.is_finite() && dt > 0.0 {
            self.desde_ultimo_tiro += dt;
        }
    }

    /// A arma já recarregou?
    pub fn pronta(&self) -> bool {
        self.desde_ultimo_tiro >= self.intervalo
    }

    /// Quanto falta para o próximo tiro, em segundos (0 se está pronta).
    pub fn recarga(&self) -> f32 {
        (self.intervalo - self.desde_ultimo_tiro).max(0.0)
    }

    /// Atira: acerta o cubo vivo mais próximo no caminho, se houver.
    ///
    /// Devolve `None` quando o cooldown ainda não passou **e** quando o tiro
    /// errou. As duas coisas são diferentes para o jogador (uma é espera, a outra
    /// é mira), mas iguais para quem chama: nos dois casos não houve abate.
    /// Quem precisa distinguir consulta [`Weapon::pronta`] antes.
    ///
    /// É o caminho de **um alvo** — o do pulso — e o que a partida usa para
    /// decidir se houve dano. Armas de área passam por [`Weapon::disparar`].
    pub fn atirar(
        &mut self,
        origem: Vec3,
        direcao: Vec3,
        pool: &mut EnemyPool,
    ) -> Option<Atingido> {
        self.disparar(origem, direcao, pool).into_iter().next()
    }

    /// Dispara a arma na mão e devolve **todos** os cubos abatidos.
    ///
    /// Um disparo do pulso devolve no máximo um; o leque devolve até três; o
    /// morteiro leva tudo o que estiver no raio da explosão. A ordem é do mais
    /// próximo ao mais longe, para o `+100` do HUD cair no alvo que o jogador
    /// mirou e não num que estava de lado.
    pub fn disparar(&mut self, origem: Vec3, direcao: Vec3, pool: &mut EnemyPool) -> Vec<Atingido> {
        if !self.pronta() {
            return Vec::new();
        }
        // A direção vem da câmera e já é unitária, mas normalizar aqui custa uma
        // raiz e garante que `distancia` seja distância para qualquer chamador.
        let tamanho = direcao.length();
        if !tamanho.is_finite() || tamanho <= 1e-6 {
            return Vec::new();
        }
        let direcao = Vec3::new(
            direcao.x / tamanho,
            direcao.y / tamanho,
            direcao.z / tamanho,
        );

        let Some(principal) = self.mais_proximo_no_raio(origem, direcao, pool) else {
            // O cooldown só é gasto quando há alvo. Um tiro que erra não pune o
            // jogador com espera por um clique perdido — em 49 fps isso é quase
            // três quadros de silêncio depois de cada erro de mira.
            return Vec::new();
        };
        self.desde_ultimo_tiro = 0.0;

        let mut atingidos = vec![principal];
        let arma = self.arma;
        if arma.alvos > 1 {
            let extras = if arma.explodir > 0.0 {
                // Explosão: quem está perto do **ponto de impacto**. Medir a
                // partir da origem pegaria os cubos enfileirados atrás do alvo.
                this_no_raio_da_explosao(origem, principal.posicao, principal.indice, arma, pool)
            } else {
                this_no_leque(origem, direcao, principal.indice, arma, pool)
            };
            atingidos.extend(extras);
        }
        atingidos.truncate(arma.alvos.max(1));

        for atingido in &atingidos {
            pool.despawn(atingido.indice);
        }
        atingidos
    }

    /// O cubo vivo mais próximo ao longo do raio, com a folga de mira aplicada.
    fn mais_proximo_no_raio(
        &self,
        origem: Vec3,
        direcao: Vec3,
        pool: &EnemyPool,
    ) -> Option<Atingido> {
        let mut melhor: Option<(usize, f32)> = None;
        for (indice, inimigo) in pool.iter_alive() {
            let mut caixa = inimigo.aabb();
            caixa.meio = Vec3::new(
                caixa.meio.x * (1.0 + FOLGA_DE_MIRA),
                caixa.meio.y * (1.0 + FOLGA_DE_MIRA),
                caixa.meio.z * (1.0 + FOLGA_DE_MIRA),
            );
            if let Some(distancia) = raycast_aabb(origem, direcao, caixa) {
                if melhor.is_none_or(|(_, atual)| distancia < atual) {
                    melhor = Some((indice, distancia));
                }
            }
        }

        let (indice, distancia) = melhor?;
        Some(Atingido {
            indice,
            distancia,
            posicao: Vec3::new(
                origem.x + direcao.x * distancia,
                origem.y + direcao.y * distancia,
                origem.z + direcao.z * distancia,
            ),
        })
    }
}

/// Cubos cujo centro está dentro do raio da explosão, do mais perto ao mais longe.
fn this_no_raio_da_explosao(
    origem: Vec3,
    impacto: Vec3,
    ja_atingido: usize,
    arma: &Arma,
    pool: &EnemyPool,
) -> Vec<Atingido> {
    let mut encontrados: Vec<(f32, Atingido)> = Vec::new();
    for (indice, inimigo) in pool.iter_alive() {
        // O alvo principal já está na lista: contá-lo de novo faria o `despawn`
        // rodar duas vezes no mesmo slot e o pool devolveria dois abates por um
        // cubo só.
        if indice == ja_atingido {
            continue;
        }
        let delta = Vec3::new(
            inimigo.position.x - impacto.x,
            inimigo.position.y - impacto.y,
            inimigo.position.z - impacto.z,
        );
        let distancia_ao_impacto = delta.length();
        if !distancia_ao_impacto.is_finite() || distancia_ao_impacto > arma.explodir {
            continue;
        }
        let delta_origem = Vec3::new(
            inimigo.position.x - origem.x,
            inimigo.position.y - origem.y,
            inimigo.position.z - origem.z,
        );
        encontrados.push((
            distancia_ao_impacto,
            Atingido {
                indice,
                // Distância do olho: é ela que a telemetria mostra e a que o
                // raio do jogador percorreu de fato.
                distancia: delta_origem.length(),
                // O `+100` nasce **no cubo**, não no ponto de impacto: numa
                // explosão, o alvo secundário está longe do centro e o número
                // subiria no vazio.
                posicao: inimigo.position,
            },
        ));
    }
    encontrados.sort_by(|a, b| a.0.total_cmp(&b.0));
    encontrados.into_iter().map(|(_, a)| a).collect()
}

/// Cubos dentro do leque, do mais alinhado ao menos alinhado.
fn this_no_leque(
    origem: Vec3,
    direcao: Vec3,
    ja_atingido: usize,
    arma: &Arma,
    pool: &EnemyPool,
) -> Vec<Atingido> {
    let mut encontrados: Vec<(f32, Atingido)> = Vec::new();
    for (indice, inimigo) in pool.iter_alive() {
        // Ver `this_no_raio_da_explosao`: o principal não entra duas vezes.
        if indice == ja_atingido {
            continue;
        }
        let delta = Vec3::new(
            inimigo.position.x - origem.x,
            inimigo.position.y - origem.y,
            inimigo.position.z - origem.z,
        );
        let distancia = delta.length();
        if !distancia.is_finite() || distancia <= 1e-6 {
            continue;
        }
        let unitario = Vec3::new(
            delta.x / distancia,
            delta.y / distancia,
            delta.z / distancia,
        );
        let cosseno = unitario.x * direcao.x + unitario.y * direcao.y + unitario.z * direcao.z;
        // `clamp` antes do `acos`: erro de arredondamento em `1.0000001` devolve
        // `NaN`, e um `NaN` numa comparação é sempre falso — o leque deixaria de
        // acertar sem nenhum erro aparecer.
        let angulo = cosseno.clamp(-1.0, 1.0).acos();
        if angulo <= arma.cone {
            encontrados.push((
                angulo,
                Atingido {
                    indice,
                    distancia,
                    posicao: Vec3::new(
                        origem.x + unitario.x * distancia,
                        origem.y + unitario.y * distancia,
                        origem.z + unitario.z * distancia,
                    ),
                },
            ));
        }
    }
    encontrados.sort_by(|a, b| a.0.total_cmp(&b.0));
    encontrados.into_iter().map(|(_, a)| a).collect()
}

impl Default for Weapon {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::procedural_mesh::generate_cube;

    /// Direção "para frente" da câmera inicial, que olha de `+Z` para `-Z`.
    const FRENTE: Vec3 = Vec3::new(0.0, 0.0, -1.0);

    fn pool_com(posicoes: &[(f32, f32, f32)]) -> EnemyPool {
        let mut pool = EnemyPool::new();
        for (x, y, z) in posicoes {
            pool.spawn(Vec3::new(*x, *y, *z), 1.0, 0.5).expect("slot");
        }
        pool
    }

    #[test]
    fn a_arma_comeca_pronta() {
        // Se começasse em cooldown, o primeiro clique do jogador não faria nada.
        let arma = Weapon::new();
        assert!(arma.pronta());
        assert_eq!(arma.recarga(), 0.0);
    }

    #[test]
    fn acerta_o_cubo_na_frente() {
        let mut arma = Weapon::new();
        let mut pool = pool_com(&[(0.0, 0.0, -10.0)]);
        let tiro = arma
            .atirar(Vec3::ZERO, FRENTE, &mut pool)
            .expect("devia acertar");
        assert_eq!(tiro.indice, 0);
        // Face do cubo em z = -10 + 0,44 (meia aresta com 10 % de folga).
        assert!(
            (tiro.distancia - 9.56).abs() < 1e-3,
            "distância {}",
            tiro.distancia
        );
        assert_eq!(pool.active_count(), 0, "o cubo atingido sai do pool");
    }

    #[test]
    fn o_impacto_fica_na_face_do_cubo() {
        let mut arma = Weapon::new();
        let mut pool = pool_com(&[(0.0, 0.0, -10.0)]);
        let tiro = arma.atirar(Vec3::ZERO, FRENTE, &mut pool).expect("acerta");
        // O ponto devolvido tem de estar sobre a caixa, não no centro dela.
        assert!((tiro.posicao.z + 9.56).abs() < 1e-3, "{:?}", tiro.posicao);
        assert!(tiro.posicao.x.abs() < 1e-5);
    }

    #[test]
    fn o_cooldown_bloqueia_o_segundo_tiro() {
        let mut arma = Weapon::new();
        let mut pool = pool_com(&[(0.0, 0.0, -10.0), (0.0, 0.0, -12.0)]);
        assert!(arma.atirar(Vec3::ZERO, FRENTE, &mut pool).is_some());
        assert!(
            arma.atirar(Vec3::ZERO, FRENTE, &mut pool).is_none(),
            "o segundo tiro saiu dentro dos 0,15 s"
        );
        assert_eq!(pool.active_count(), 1, "só um cubo podia ter morrido");
    }

    #[test]
    fn depois_do_intervalo_a_arma_volta() {
        let mut arma = Weapon::new();
        let mut pool = pool_com(&[(0.0, 0.0, -10.0), (0.0, 0.0, -12.0)]);
        arma.atirar(Vec3::ZERO, FRENTE, &mut pool);
        arma.atualizar(INTERVALO - 0.01);
        assert!(arma.atirar(Vec3::ZERO, FRENTE, &mut pool).is_none());
        arma.atualizar(0.02);
        assert!(arma.atirar(Vec3::ZERO, FRENTE, &mut pool).is_some());
    }

    #[test]
    fn mira_em_dois_cubos_alinhados_mata_o_da_frente() {
        // A ordem do pool não pode decidir qual morre: o mais próximo vence.
        let mut arma = Weapon::new();
        let mut pool = pool_com(&[(0.0, 0.0, -14.0), (0.0, 0.0, -9.0)]);
        let tiro = arma.atirar(Vec3::ZERO, FRENTE, &mut pool).expect("acerta");
        assert_eq!(tiro.indice, 1, "devia matar o cubo de -9, não o de -14");
        assert!((tiro.distancia - 8.56).abs() < 1e-3);
    }

    #[test]
    fn erra_quando_nao_ha_nada_no_caminho() {
        let mut arma = Weapon::new();
        let mut pool = pool_com(&[(5.0, 0.0, -10.0)]);
        assert!(arma.atirar(Vec3::ZERO, FRENTE, &mut pool).is_none());
        assert_eq!(pool.active_count(), 1, "errar não pode matar ninguém");
    }

    #[test]
    fn errar_nao_gasta_o_cooldown() {
        // 0,15 s a 49 fps são quase três quadros de silêncio depois de cada erro
        // de mira. Punir o erro com espera transforma mira ruim em travamento.
        let mut arma = Weapon::new();
        let mut pool = pool_com(&[(5.0, 0.0, -10.0)]);
        assert!(arma.atirar(Vec3::ZERO, FRENTE, &mut pool).is_none());
        assert!(arma.pronta(), "o erro gastou o cooldown");
        let mut pool2 = pool_com(&[(0.0, 0.0, -10.0)]);
        assert!(arma.atirar(Vec3::ZERO, FRENTE, &mut pool2).is_some());
    }

    #[test]
    fn atira_para_tras_e_nao_acerta_o_que_esta_na_frente() {
        let mut arma = Weapon::new();
        let mut pool = pool_com(&[(0.0, 0.0, -10.0)]);
        assert!(arma
            .atirar(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), &mut pool)
            .is_none());
    }

    #[test]
    fn atira_de_dentro_do_cubo() {
        let mut arma = Weapon::new();
        let mut pool = pool_com(&[(0.0, 0.0, -1.0)]);
        assert!(arma.atirar(Vec3::ZERO, FRENTE, &mut pool).is_some());
    }

    #[test]
    fn direcao_invalida_nao_atira_nem_gasta_o_cooldown() {
        let mut arma = Weapon::new();
        let mut pool = pool_com(&[(0.0, 0.0, -10.0)]);
        assert!(arma
            .atirar(Vec3::ZERO, Vec3::new(f32::NAN, 0.0, -1.0), &mut pool)
            .is_none());
        assert!(arma.atirar(Vec3::ZERO, Vec3::ZERO, &mut pool).is_none());
        assert!(arma.pronta());
    }

    #[test]
    fn direcao_nao_normalizada_e_aceita_e_a_distancia_continua_em_unidades() {
        // O chamador pode passar o vetor da câmera já normalizado; se algum dia
        // passar outro, a distância não pode virar "parâmetro da reta".
        let mut arma = Weapon::new();
        let mut pool = pool_com(&[(0.0, 0.0, -10.0)]);
        let tiro = arma
            .atirar(Vec3::ZERO, Vec3::new(0.0, 0.0, -3.0), &mut pool)
            .expect("acerta");
        assert!(
            (tiro.distancia - 9.56).abs() < 1e-3,
            "distância {}",
            tiro.distancia
        );
    }

    #[test]
    fn o_pool_vazio_nao_acerta_nada() {
        let mut arma = Weapon::new();
        let mut pool = EnemyPool::new();
        assert!(arma.atirar(Vec3::ZERO, FRENTE, &mut pool).is_none());
    }

    #[test]
    fn a_folga_de_mira_aumenta_a_caixa_em_dez_por_cento() {
        // Medido pela lateral: com folga, um raio que passa a 0,43 do centro
        // acerta (meia aresta 0,4 + 10 %); sem folga, erraria.
        let mut arma = Weapon::new();
        let mut pool = pool_com(&[(0.43, 0.0, -10.0)]);
        assert!(
            arma.atirar(Vec3::ZERO, FRENTE, &mut pool).is_some(),
            "a folga de 10 % não foi aplicada"
        );

        let mut arma = Weapon::new();
        let mut pool = pool_com(&[(0.55, 0.0, -10.0)]);
        assert!(
            arma.atirar(Vec3::ZERO, FRENTE, &mut pool).is_none(),
            "a folga não pode virar um ímã de 0,55"
        );
    }

    #[test]
    fn o_cubo_desenhado_tem_o_lado_que_a_mira_usa() {
        // Se alguém mudar o lado do cubo e esquecer a mira, a folga passa a
        // mentir. Amarrar os dois pelo valor é o que evita isso.
        let cubo = generate_cube(crate::gameplay::enemy::LADO_PADRAO);
        let (minimo, maximo) = cubo.limites();
        let lado = maximo[0] - minimo[0];
        assert!((lado - crate::gameplay::enemy::LADO_PADRAO).abs() < 1e-5);
    }

    #[test]
    fn o_pulso_e_a_arma_do_bloco_2a() {
        // A regressão que este teste trava: trocar de arma não pode mudar o
        // comportamento da partida que foi medida em 2a/3b.
        assert_eq!(PULSAR.intervalo, INTERVALO);
        assert_eq!(PULSAR.alvos, 1);
        assert_eq!(PULSAR.cone, 0.0);
        assert_eq!(PULSAR.explodir, 0.0);
        assert_eq!(PULSAR.pontos, PONTOS_POR_ABATE);
        assert_eq!(Weapon::new().arma.id, PULSAR.id);
    }

    #[test]
    fn as_tres_armas_sao_diferentes_e_ordenadas_por_cadencia() {
        assert_eq!(ARMAS.len(), 3);
        // Cada uma tem de ser diferente em algo que o jogador sinta, senão é a
        // mesma arma com outro nome.
        for (i, a) in ARMAS.iter().enumerate() {
            for b in ARMAS.iter().skip(i + 1) {
                assert_ne!(a.id, b.id);
                assert!(
                    a.intervalo != b.intervalo || a.cone != b.cone || a.explodir != b.explodir,
                    "{} e {} são a mesma arma",
                    a.nome,
                    b.nome
                );
            }
        }
        // Quanto mais área a arma cobre, mais lenta ela é: sem essa troca, a de
        // área seria estritamente melhor e ninguém usaria as outras. Em bloco
        // `const` para a checagem valer na compilação, e não só no teste.
        const {
            assert!(PULSAR.intervalo < LEQUE.intervalo);
            assert!(LEQUE.intervalo < MORTEIRO.intervalo);
        }
    }

    #[test]
    fn o_leque_pega_tres_cubos_lado_a_lado() {
        // Três cubos a 2°, 4° e 10° do centro do raio. O leque (9°) leva os dois
        // primeiros e deixa o terceiro; o pulso levaria só o do centro.
        let mut pool = pool_com(&[(0.0, 0.0, -10.0), (0.35, 0.0, -10.0), (1.76, 0.0, -10.0)]);
        let mut arma = Weapon::com_arma(&LEQUE);
        let atingidos = arma.disparar(Vec3::ZERO, FRENTE, &mut pool);
        assert_eq!(atingidos.len(), 2, "o leque devia levar dois");
        assert_eq!(pool.active_count(), 1, "o de 10° fica");

        let mut pool = pool_com(&[(0.0, 0.0, -10.0), (0.35, 0.0, -10.0)]);
        let mut pulso = Weapon::new();
        assert_eq!(pulso.disparar(Vec3::ZERO, FRENTE, &mut pool).len(), 1);
        assert_eq!(pool.active_count(), 1, "o pulso leva um só");
    }

    #[test]
    fn o_morteiro_leva_o_que_estiver_perto_do_impacto() {
        // O alvo em -10 é o que o raio acerta; o de -11 está a 1 unidade do
        // impacto e morre junto. O de -30 está fora do raio de 3,2.
        let mut pool = pool_com(&[(0.0, 0.0, -10.0), (0.5, 0.0, -11.0), (0.0, 0.0, -30.0)]);
        let mut arma = Weapon::com_arma(&MORTEIRO);
        let atingidos = arma.disparar(Vec3::ZERO, FRENTE, &mut pool);
        assert_eq!(atingidos.len(), 2, "o morteiro devia levar dois");
        assert_eq!(pool.active_count(), 1);
        // O primeiro da lista é sempre o que o jogador mirou.
        assert!(
            (atingidos[0].distancia - 9.56).abs() < 1e-3,
            "primeiro a {:.2}",
            atingidos[0].distancia
        );
    }

    #[test]
    fn a_arma_de_area_respeita_o_teto_de_alvos() {
        // Nove cubos amontoados: o leque tem teto de três, e sem o `truncate` a
        // arma abateria o pool inteiro com um clique.
        let mut posicoes = Vec::new();
        for i in 0..9 {
            posicoes.push((0.05 * i as f32, 0.0, -10.0));
        }
        let mut pool = pool_com(&posicoes);
        let mut arma = Weapon::com_arma(&LEQUE);
        let atingidos = arma.disparar(Vec3::ZERO, FRENTE, &mut pool);
        assert_eq!(atingidos.len(), LEQUE.alvos);
        assert_eq!(pool.active_count(), 9 - LEQUE.alvos);
    }

    #[test]
    fn a_area_nao_gasta_o_cooldown_quando_erra() {
        // Mesma regra do pulso: um clique perdido não custa 0,7 s de silêncio.
        let mut pool = pool_com(&[(0.0, 50.0, -10.0)]);
        let mut arma = Weapon::com_arma(&MORTEIRO);
        assert!(arma.disparar(Vec3::ZERO, FRENTE, &mut pool).is_empty());
        assert!(arma.pronta(), "o tiro errado não pode gastar a recarga");
    }

    #[test]
    fn trocar_de_arma_nao_pune_o_jogador() {
        // Se a troca zerasse o cooldown, o jogador apertaria o gatilho e nada
        // sairia por 0,7 s, sem nenhum sinal na tela.
        let mut arma = Weapon::new();
        arma.atualizar(1.0);
        assert!(arma.pronta());
        arma.trocar(&MORTEIRO);
        assert_eq!(arma.intervalo, MORTEIRO.intervalo);
        assert!(arma.pronta(), "a troca não pode deixar a arma travada");

        let mut pool = pool_com(&[(0.0, 0.0, -10.0)]);
        assert_eq!(arma.disparar(Vec3::ZERO, FRENTE, &mut pool).len(), 1);
    }

    #[test]
    fn a_troca_percorre_as_tres_em_laco() {
        let mut arma = Weapon::new();
        let mut vistos = vec![arma.arma.id];
        for _ in 0..3 {
            arma.proxima();
            vistos.push(arma.arma.id);
        }
        assert_eq!(vistos, vec!["pulsar", "leque", "morteiro", "pulsar"]);
        assert_eq!(arma.indice(), 0);

        // Escolher pelo índice da tabela é o caminho do `--arma`.
        arma.trocar_indice(2);
        assert_eq!(arma.arma.id, MORTEIRO.id);
        arma.trocar_indice(7); // dá a volta
        assert_eq!(arma.arma.id, ARMAS[1].id);
    }

    #[test]
    fn o_identificador_da_arma_e_lido() {
        assert_eq!(indice_da_arma("pulsar"), Some(0));
        assert_eq!(indice_da_arma("  MORTEIRO "), Some(2));
        assert_eq!(indice_da_arma("canhao"), None);
        for (i, arma) in ARMAS.iter().enumerate() {
            assert_eq!(indice_da_arma(arma.id), Some(i));
        }
    }

    #[test]
    fn os_pontos_por_abate_vem_da_arma() {
        // O `+100` do HUD era uma constante; com três armas ele tem de acompanhar
        // a que está na mão, senão o placar mentiria sobre o que o jogador fez.
        let mut arma = Weapon::new();
        assert_eq!(arma.pontos(), PONTOS_POR_ABATE);
        arma.trocar(&MORTEIRO);
        assert_eq!(arma.pontos(), MORTEIRO.pontos);
        assert_ne!(arma.pontos(), PONTOS_POR_ABATE);
    }
}
