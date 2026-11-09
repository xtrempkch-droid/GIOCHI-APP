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

use crate::gameplay::collision::raycast_aabb;
use crate::gameplay::enemy_pool::EnemyPool;
use giochi_engine_3d::math3d::Vec3;

/// Intervalo mínimo entre dois tiros, em segundos.
pub const INTERVALO: f32 = 0.15;
/// Pontos por cubo abatido.
pub const PONTOS_POR_ABATE: u32 = 100;
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
    /// Tempo desde o último tiro, em segundos.
    pub desde_ultimo_tiro: f32,
    /// Intervalo configurado (permite ajuste por dificuldade sem tocar no resto).
    pub intervalo: f32,
}

impl Weapon {
    /// Arma pronta para atirar.
    pub fn new() -> Self {
        Self {
            // Começa "carregada": o primeiro clique tem de atirar, e um cooldown
            // inicial de 0,15 s faria o primeiro tiro do jogador se perder.
            desde_ultimo_tiro: INTERVALO,
            intervalo: INTERVALO,
        }
    }

    /// Arma com outro intervalo.
    pub fn com_intervalo(intervalo: f32) -> Self {
        let intervalo = if intervalo.is_finite() && intervalo > 0.0 {
            intervalo
        } else {
            INTERVALO
        };
        Self {
            desde_ultimo_tiro: intervalo,
            intervalo,
        }
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
    pub fn atirar(
        &mut self,
        origem: Vec3,
        direcao: Vec3,
        pool: &mut EnemyPool,
    ) -> Option<Atingido> {
        if !self.pronta() {
            return None;
        }
        // A direção vem da câmera e já é unitária, mas normalizar aqui custa uma
        // raiz e garante que `distancia` seja distância para qualquer chamador.
        let tamanho = direcao.length();
        if !tamanho.is_finite() || tamanho <= 1e-6 {
            return None;
        }
        let direcao = Vec3::new(
            direcao.x / tamanho,
            direcao.y / tamanho,
            direcao.z / tamanho,
        );

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

        // O cooldown só é gasto quando há alvo. Um tiro que erra não pune o
        // jogador com 0,15 s de espera por um clique perdido — em 49 fps isso é
        // quase três quadros de silêncio depois de cada erro de mira.
        let (indice, distancia) = melhor?;
        self.desde_ultimo_tiro = 0.0;
        pool.despawn(indice);

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
}
