//! A partida em andamento: quem nasce, para onde vai e o que dói.
//!
//! ## Simulação separada do desenho
//!
//! [`Partida`] não conhece GL: ela tem o pool, o sorteio e o relógio de spawn, e
//! é onde a regra ("chegou no asteroide, perdeu uma vida") vive. [`Inimigos`] tem
//! a malha e o shader e só desenha.
//!
//! A separação não é preciosismo: é o que permite testar o spawn e o dano no CI,
//! onde não existe contexto gráfico. O plano chamava isto de `GameplayScene`; a
//! cena de verdade (com pausa e menu) chega no bloco 3a, e até lá o nome honesto
//! é `Partida`.

use crate::gameplay::collision::sphere_aabb_intersect;
use crate::gameplay::enemy_pool::{EnemyPool, CAPACIDADE};
use crate::state::GameState;
use crate::utils::procedural_mesh::generate_cube;
use crate::utils::rng::Sorteio;
use giochi_engine_3d::math3d::{Camera3D, Color3D, Transform3D, Vec3};
use giochi_engine_3d::render3d::{MeshHandle, RenderContext3D, ShaderHandle};
use giochi_engine_3d::shader::BuiltinShader;
use giochi_engine_3d::Result3D;

/// De quanto em quanto tempo nasce um cubo (bloco 1b: um a cada 2 s).
pub const INTERVALO_SPAWN: f32 = 2.0;
/// Velocidade dos cubos, em unidades por segundo.
pub const VELOCIDADE: f32 = 3.0;
/// Raio mínimo de spawn.
pub const RAIO_SPAWN_MIN: f32 = 15.0;
/// Raio máximo de spawn.
pub const RAIO_SPAWN_MAX: f32 = 20.0;
/// Meia-altura da faixa de spawn em torno do horizonte.
///
/// Nascer uniformemente na esfera põe metade dos cubos fora do campo de visão
/// (que cobre ~12 % dela) e eles chegam sem que o jogador veja de onde. Uma faixa
/// de ±4 unidades mantém todos no plano da câmera — é o que um tower defense
/// precisa para o jogador entender de onde vem a ameaça.
pub const ALTURA_SPAWN: f32 = 4.0;
/// Raio da esfera de colisão do asteroide.
///
/// 2,5 e não os 2,0 nominais: a malha distorcida chega a 2,44, e um cubo que
/// encosta na ponta da rocha tem de contar como dano. O valor dobra como margem
/// de leitura — o cubo de 0,8 tem 0,4 de meia-aresta.
pub const RAIO_COLISAO_ASTEROIDE: f32 = 2.5;
/// Cor do cubo hostil.
pub const COR_INIMIGO: Color3D = Color3D::new(0.88, 0.26, 0.18, 1.0);
/// Passo máximo de uma subdivisão da simulação.
///
/// É o passo fixo do laço. Um `dt` maior que isto vira vários passos, para o
/// resultado não depender de como o tempo chegou.
pub const PASSO_MAX: f32 = 1.0 / 60.0;

/// A simulação da partida.
#[derive(Debug, Clone, PartialEq)]
pub struct Partida {
    pool: EnemyPool,
    sorteio: Sorteio,
    desde_ultimo_spawn: f32,
    game_over_avisado: bool,
    derrotas: u32,
}

impl Partida {
    /// Partida nova, com a semente do sorteio de posições.
    pub fn new(semente: u64) -> Self {
        Self {
            pool: EnemyPool::new(),
            sorteio: Sorteio::new(semente),
            desde_ultimo_spawn: 0.0,
            game_over_avisado: false,
            derrotas: 0,
        }
    }

    /// Avança a simulação por `dt` segundos.
    ///
    /// O passo é subdividido em pedaços de no máximo [`PASSO_MAX`]. Sem isso, um
    /// `dt` grande moveria por inteiro até os cubos que nasceram **no meio** do
    /// intervalo — e o `--simular` do diagnóstico mediria uma partida que o laço
    /// nunca produz. Com a subdivisão, `atualizar(t)` é idêntico a `t · 60`
    /// chamadas de `1/60`, e há teste comparando as duas.
    pub fn atualizar(&mut self, dt: f32, estado: &mut GameState) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let mut restante = dt;
        while restante > 0.0 {
            let passo = restante.min(PASSO_MAX);
            self.passo(passo, estado);
            restante -= passo;
            if estado.acabou() {
                return;
            }
        }
    }

    /// Um passo de simulação já subdividido.
    fn passo(&mut self, dt: f32, estado: &mut GameState) {
        if estado.acabou() {
            return;
        }

        self.desde_ultimo_spawn += dt;
        while self.desde_ultimo_spawn >= INTERVALO_SPAWN {
            self.desde_ultimo_spawn -= INTERVALO_SPAWN;
            if !self.pool.tem_espaco() {
                // Pool cheio: o tempo fica acumulado e o próximo nasce assim que
                // houver slot. Descartar o acumulado faria a onda "pular" um
                // cubo por causa de uma morte no instante errado.
                break;
            }
            let posicao = self.posicao_de_spawn();
            if self.pool.spawn(posicao, VELOCIDADE).is_none() {
                break;
            }
            log::debug!(
                "inimigo em {:.1}, {:.1}, {:.1}",
                posicao.x,
                posicao.y,
                posicao.z
            );
        }

        self.mover_e_colidir(dt, estado);
    }

    /// Sorteia um ponto na casca `RAIO_SPAWN_MIN..RAIO_SPAWN_MAX`, na faixa do
    /// horizonte.
    fn posicao_de_spawn(&mut self) -> Vec3 {
        let raio = self.sorteio.faixa(RAIO_SPAWN_MIN, RAIO_SPAWN_MAX);
        let altura = self.sorteio.faixa(-ALTURA_SPAWN, ALTURA_SPAWN);
        let angulo = self.sorteio.faixa(0.0, std::f32::consts::TAU);
        let anel = (raio * raio - altura * altura).max(0.0).sqrt();
        Vec3::new(anel * angulo.cos(), altura, anel * angulo.sin())
    }

    /// Move todos e resolve o dano de quem chegou.
    fn mover_e_colidir(&mut self, dt: f32, estado: &mut GameState) {
        let alvo = Vec3::ZERO;
        let mut atingidos: Vec<usize> = Vec::new();

        for (indice, inimigo) in self.pool.iter_alive_mut() {
            inimigo.update(dt, alvo);
            if sphere_aabb_intersect(alvo, RAIO_COLISAO_ASTEROIDE, inimigo.aabb()) {
                atingidos.push(indice);
            }
        }

        // Fora do laço: `despawn` pede `&mut self.pool`, que o iterador acima
        // ainda empresta.
        for indice in atingidos {
            if self.pool.despawn(indice) {
                self.derrotas += 1;
                let acabou = estado.perdeu_vida();
                log::info!("player damaged, lives: {}", estado.lives);
                if acabou && !self.game_over_avisado {
                    // Bloco 1b: a partida termina aqui, só no log. A cena de
                    // derrota é do 3a.
                    log::info!("GAME OVER");
                    self.game_over_avisado = true;
                }
            }
        }
    }

    /// Quantos cubos estão no ar.
    pub fn inimigos(&self) -> usize {
        self.pool.active_count()
    }

    /// Quantos inimigos podem existir ao mesmo tempo.
    pub fn capacidade(&self) -> usize {
        CAPACIDADE
    }

    /// Quantos cubos já chegaram no asteroide (teste e diagnóstico).
    pub fn derrotas(&self) -> u32 {
        self.derrotas
    }

    /// O pool, para leitura por quem desenha.
    pub fn pool(&self) -> &EnemyPool {
        &self.pool
    }

    /// Reinício de partida.
    pub fn limpar(&mut self) {
        self.pool.limpar();
        self.desde_ultimo_spawn = 0.0;
        self.game_over_avisado = false;
        self.derrotas = 0;
    }
}

/// Os recursos de GPU para desenhar os cubos.
///
/// Uma malha e um shader para todos: 15 cubos são 15 desenhos, não 15 malhas.
pub struct Inimigos {
    malha: MeshHandle,
    shader: ShaderHandle,
}

impl Inimigos {
    /// Cria a malha do cubo e o shader `flat`.
    pub fn new(render: &mut RenderContext3D) -> Result3D<Self> {
        let malha = generate_cube(crate::gameplay::enemy::LADO_PADRAO);
        let handle = render.create_mesh(&malha.vertices, &malha.indices)?;
        let shader = render.create_builtin_shader(BuiltinShader::Flat)?;
        Ok(Self {
            malha: handle,
            shader,
        })
    }

    /// Desenha cada cubo vivo na sua posição.
    pub fn desenhar(
        &self,
        render: &mut RenderContext3D,
        partida: &Partida,
        camera: &Camera3D,
    ) -> Result3D<()> {
        for (_, inimigo) in partida.pool().iter_alive() {
            render.draw_mesh_colored(
                self.malha,
                self.shader,
                &Transform3D::from_translation(inimigo.position),
                camera,
                COR_INIMIGO,
            )?;
        }
        Ok(())
    }

    /// Devolve os recursos à engine.
    pub fn destruir(self, render: &mut RenderContext3D) {
        render.destroy_mesh(self.malha);
        render.destroy_shader(self.shader);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::VIDAS_INICIAIS;

    fn partida() -> (Partida, GameState) {
        (Partida::new(1234), GameState::new())
    }

    #[test]
    fn nasce_um_cubo_a_cada_dois_segundos() {
        // As folgas de 0,05 s existem porque `INTERVALO_SPAWN` é comparado com
        // uma soma de passos de 1/60 s, e exigir o instante exato faria o teste
        // depender do arredondamento do `f32` em vez da regra.
        let (mut partida, mut estado) = partida();
        partida.atualizar(1.9, &mut estado);
        assert_eq!(partida.inimigos(), 0, "1,9 s ainda não é hora");
        partida.atualizar(0.15, &mut estado);
        assert_eq!(partida.inimigos(), 1, "2,05 s deviam dar um cubo");
        partida.atualizar(2.0, &mut estado);
        assert_eq!(partida.inimigos(), 2);
    }

    #[test]
    fn um_passo_longo_e_igual_a_muitos_curtos() {
        // É a propriedade que torna o `--simular` confiável: 6 s de uma vez têm
        // de dar exatamente o mesmo que 360 passos de 1/60 s.
        let (mut longo, mut estado_longo) = (Partida::new(1234), GameState::new());
        let (mut curto, mut estado_curto) = (Partida::new(1234), GameState::new());

        longo.atualizar(6.0, &mut estado_longo);
        for _ in 0..360 {
            curto.atualizar(PASSO_MAX, &mut estado_curto);
        }

        assert_eq!(longo.derrotas(), curto.derrotas());
        assert_eq!(longo.inimigos(), curto.inimigos());
        assert_eq!(estado_longo, estado_curto);

        // Posições **equivalentes**, não bit-idênticas: 360 somas de `1/60` em
        // `f32` não dão o mesmo que a subdivisão interna do mesmo total, e
        // exigir igualdade exata mediria o arredondamento em vez da regra.
        let posicoes = |partida: &Partida| -> Vec<[f32; 3]> {
            partida
                .pool()
                .iter_alive()
                .map(|(_, inimigo)| inimigo.position.to_array())
                .collect()
        };
        for (a, b) in posicoes(&longo).iter().zip(posicoes(&curto).iter()) {
            for eixo in 0..3 {
                assert!((a[eixo] - b[eixo]).abs() < 1e-3, "{a:?} contra {b:?}");
            }
        }
    }

    #[test]
    fn a_fila_nao_estoura_o_pool() {
        let (mut partida, mut estado) = partida();
        // 200 s dariam 100 cubos; o pool tem 15 e todos chegam ao asteroide,
        // então o que importa é nunca passar da capacidade.
        for _ in 0..200 {
            partida.atualizar(1.0, &mut estado);
            assert!(
                partida.inimigos() <= partida.capacidade(),
                "{} cubos com capacidade {}",
                partida.inimigos(),
                partida.capacidade()
            );
        }
    }

    #[test]
    fn o_cubo_nasce_na_casca_e_na_faixa() {
        let (mut partida, _) = partida();
        for _ in 0..200 {
            let posicao = partida.posicao_de_spawn();
            let raio =
                (posicao.x * posicao.x + posicao.y * posicao.y + posicao.z * posicao.z).sqrt();
            assert!(
                (RAIO_SPAWN_MIN - 1e-3..=RAIO_SPAWN_MAX + 1e-3).contains(&raio),
                "raio {raio} fora de [{RAIO_SPAWN_MIN}, {RAIO_SPAWN_MAX}]"
            );
            assert!(
                posicao.y.abs() <= ALTURA_SPAWN + 1e-3,
                "altura {} fora da faixa",
                posicao.y
            );
        }
    }

    #[test]
    fn o_cubo_que_chega_tira_uma_vida_e_some() {
        let (mut partida, mut estado) = partida();
        // Roda em passos fixos até a primeira chegada: nasce entre 15 e 20
        // unidades a 3 u/s, então chega entre 5 e 6,7 s depois de nascer.
        let mut passos = 0;
        while partida.derrotas() == 0 && passos < 900 {
            partida.atualizar(PASSO_MAX, &mut estado);
            passos += 1;
        }
        assert_eq!(partida.derrotas(), 1, "um cubo devia ter chegado");
        assert_eq!(estado.lives, VIDAS_INICIAIS - 1);

        // O que tem de ser verdade não é "o pool está vazio" — outros cubos já
        // nasceram e estão a caminho —, e sim que nenhum colidindo ficou no ar.
        for (_, inimigo) in partida.pool().iter_alive() {
            assert!(
                !sphere_aabb_intersect(Vec3::ZERO, RAIO_COLISAO_ASTEROIDE, inimigo.aabb()),
                "um cubo já colidindo continuou vivo em {:?}",
                inimigo.position
            );
        }
    }

    #[test]
    fn o_jogo_termina_quando_as_vidas_acabam() {
        let (mut partida, mut estado) = partida();
        for _ in 0..200 {
            partida.atualizar(1.0, &mut estado);
            if estado.acabou() {
                break;
            }
        }
        assert!(estado.acabou(), "200 s deviam bastar para perder 5 vidas");
        assert_eq!(estado.lives, 0);
    }

    #[test]
    fn depois_do_fim_nada_mais_acontece() {
        let (mut partida, mut estado) = partida();
        estado.lives = 0;
        partida.atualizar(10.0, &mut estado);
        assert_eq!(partida.inimigos(), 0, "não devia nascer nada após o fim");
        assert_eq!(estado.lives, 0);
    }

    #[test]
    fn a_partida_e_deterministica_com_a_mesma_semente() {
        // É o que permite comparar duas capturas e saber que a diferença é do
        // código, não do sorteio.
        let (mut a, mut estado_a) = (Partida::new(7), GameState::new());
        let (mut b, mut estado_b) = (Partida::new(7), GameState::new());
        for _ in 0..50 {
            a.atualizar(0.5, &mut estado_a);
            b.atualizar(0.5, &mut estado_b);
        }
        assert_eq!(a, b);
        assert_eq!(estado_a, estado_b);
    }

    #[test]
    fn sementes_diferentes_espalham_diferente() {
        let (mut a, mut estado_a) = (Partida::new(1), GameState::new());
        let (mut b, mut estado_b) = (Partida::new(2), GameState::new());
        a.atualizar(INTERVALO_SPAWN, &mut estado_a);
        b.atualizar(INTERVALO_SPAWN, &mut estado_b);
        assert_ne!(a, b);
    }

    #[test]
    fn dt_invalido_nao_mexe_em_nada() {
        let (mut partida, mut estado) = partida();
        let antes = partida.clone();
        partida.atualizar(0.0, &mut estado);
        partida.atualizar(-5.0, &mut estado);
        partida.atualizar(f32::NAN, &mut estado);
        assert_eq!(partida, antes);
    }

    #[test]
    fn limpar_devolve_a_partida_ao_inicio() {
        let (mut partida, mut estado) = partida();
        partida.atualizar(10.0, &mut estado);
        partida.limpar();
        assert_eq!(partida.inimigos(), 0);
        assert_eq!(partida.derrotas(), 0);
    }

    #[test]
    fn a_capacidade_do_pool_e_a_do_plano() {
        // 15 do R5. Se alguém mudar um sem o outro, o orçamento de performance
        // deixa de valer em silêncio.
        assert_eq!(CAPACIDADE, 15);
        assert_eq!(Partida::new(0).capacidade(), 15);
    }

    #[test]
    fn perder_todas_as_vidas_avisa_uma_vez_so() {
        // `game_over_avisado` existe para o log não repetir a cada cubo que
        // chega depois do fim.
        let (mut partida, mut estado) = partida();
        partida.atualizar(200.0, &mut estado);
        assert!(estado.acabou());
        assert_eq!(estado.lives, 0);
        assert!(partida.derrotas() >= VIDAS_INICIAIS as u32);
        assert!(partida.game_over_avisado);
    }
}
