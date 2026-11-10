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

use crate::gameplay::campo::Campo;
use crate::gameplay::collision::sphere_aabb_intersect;
use crate::gameplay::dificuldade;
use crate::gameplay::enemy_pool::{EnemyPool, CAPACIDADE};
use crate::gameplay::modo::Modo;
use crate::gameplay::weapon::{Atingido, Weapon};
use crate::state::GameState;
use crate::utils::procedural_mesh::generate_cube;
use crate::utils::rng::Sorteio;
use giochi_engine_3d::math3d::{Camera3D, Transform3D, Vec3};
use giochi_engine_3d::render3d::{MeshHandle, RenderContext3D, ShaderHandle};
use giochi_engine_3d::shader::BuiltinShader;
use giochi_engine_3d::Result3D;

/// De quanto em quanto tempo nasce um cubo na **primeira** onda.
///
/// O valor por onda vem de [`crate::gameplay::dificuldade::da_onda`]; esta
/// constante é o piso do bloco 1b, mantido porque é ele que a onda 1 usa e
/// porque os testes do 1b comparam com ela.
pub const INTERVALO_SPAWN: f32 = 2.0;
/// Velocidade dos cubos na **primeira** onda, em unidades por segundo.
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
    game_over: bool,
    derrotas: u32,
    /// Onde os cubos nascem — derivado da câmera, ver [`Campo`].
    campo: Campo,
    /// Modo de dificuldade em vigor.
    modo: Modo,
}

impl Partida {
    /// Partida nova, com a semente do sorteio de posições.
    pub fn new(semente: u64) -> Self {
        Self {
            pool: EnemyPool::new(),
            sorteio: Sorteio::new(semente),
            desde_ultimo_spawn: 0.0,
            game_over_avisado: false,
            game_over: false,
            derrotas: 0,
            campo: Campo::padrao(),
            modo: Modo::default(),
        }
    }

    /// Partida com o campo e o modo do jogo.
    ///
    /// Os dois juntos e não separados porque um depende do outro: o campo vem da
    /// distância da câmera, que é a do modo. Receber só um deles deixaria a
    /// partida com cubos nascendo dentro da câmera no modo fácil.
    pub fn com_modo(semente: u64, modo: Modo) -> Self {
        let mut partida = Self::new(semente);
        partida.definir_modo(modo);
        partida
    }

    /// Troca o modo, recalculando o campo.
    pub fn definir_modo(&mut self, modo: Modo) {
        self.modo = modo;
        self.campo = Campo::para_distancia(modo.distancia_da_camera());
    }

    /// Troca o campo sem mexer no modo.
    ///
    /// O laço chama isto a cada quadro com a distância **real** da câmera: o
    /// jogador pode dar zoom, e a casca tem de continuar à frente do olho. Se só
    /// o modo definisse o campo, dar zoom para 15 no modo difícil poria o olho
    /// exatamente sobre a casca de `15..20`.
    pub fn definir_campo(&mut self, campo: Campo) {
        self.campo = campo;
    }

    /// Modo em vigor.
    pub fn modo(&self) -> Modo {
        self.modo
    }

    /// Campo de spawn em vigor.
    pub fn campo(&self) -> Campo {
        self.campo
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

        // A dificuldade vem da **onda atual** e do modo, não de uma constante: era
        // isso que fazia a onda 3 jogar igual à 1.
        let onda = dificuldade::da_onda(estado.wave).com_modo(self.modo);
        self.desde_ultimo_spawn += dt;
        while self.desde_ultimo_spawn >= onda.intervalo {
            self.desde_ultimo_spawn -= onda.intervalo;
            for _ in 0..onda.por_vez {
                if !self.pool.tem_espaco() {
                    // Pool cheio: o tempo fica acumulado e o próximo nasce
                    // assim que houver slot. Descartar o acumulado faria a onda
                    // "pular" um cubo por causa de uma morte no instante errado.
                    break;
                }
                let posicao = self.posicao_de_spawn();
                let variacao = self.sorteio.faixa(0.0, 1.0);
                if self
                    .pool
                    .spawn(posicao, onda.velocidade, variacao)
                    .is_none()
                {
                    break;
                }
                log::debug!(
                    "inimigo em {:.1}, {:.1}, {:.1} a {:.1} m/s",
                    posicao.x,
                    posicao.y,
                    posicao.z,
                    onda.velocidade
                );
            }
        }

        self.mover_e_colidir(dt, estado);
    }

    /// Sorteia um ponto na casca do campo, na faixa do horizonte.
    ///
    /// Os raios vêm de [`Partida::campo`] e não das constantes: no modo fácil a
    /// câmera está a 18 e a casca fixa de `15..20` a envolveria.
    fn posicao_de_spawn(&mut self) -> Vec3 {
        let raio = self.sorteio.faixa(self.campo.raio_min, self.campo.raio_max);
        let altura = self.sorteio.faixa(-self.campo.altura, self.campo.altura);
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
                    self.game_over = true;
                }
            }
        }
    }

    /// Enche o pool na hora, para diagnóstico e medição.
    ///
    /// Existe porque medir "15 cubos a 30 fps" exige 15 cubos **agora**: esperar
    /// 30 s de spawn a 2 s gastaria mais tempo do que a medição e ainda deixaria
    /// a cena dependente de mortes no meio.
    ///
    /// Devolve quantos couberam.
    pub fn povoar(&mut self, quantos: u32) -> usize {
        let mut criados = 0;
        for _ in 0..quantos {
            if !self.pool.tem_espaco() {
                break;
            }
            let posicao = self.posicao_de_spawn();
            let variacao = self.sorteio.faixa(0.0, 1.0);
            let velocidade = VELOCIDADE * self.modo.fator_velocidade();
            if self.pool.spawn(posicao, velocidade, variacao).is_some() {
                criados += 1;
            }
        }
        // O relógio de spawn não é zerado: os cubos criados por aqui entraram no
        // lugar dos que o tempo ainda traria, e manter o acumulado evita que os
        // 15 primeiros segundos de uma corrida de medição disparem uma rajada.
        criados
    }

    /// Planta um cubo parado numa posição exata.
    ///
    /// É ferramenta de diagnóstico, não de jogo: serve ao `--alvo`, que põe
    /// cubos **no eixo da câmera** para que o tiro do centro da tela tenha alvo
    /// certo. Sem isso, provar "atirar no cubo o mata" exigiria que o sorteio
    /// caísse na linha de mira, e um teste que depende do sorteio não prova.
    ///
    /// Velocidade zero de propósito: um cubo em movimento sairia da mira entre o
    /// primeiro e o terceiro tiro, e o que se quer medir é a arma, não a mira.
    pub fn plantar(&mut self, posicao: Vec3, variacao: f32) -> Option<usize> {
        self.pool.spawn(posicao, 0.0, variacao)
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

    /// Resolve um tiro: delega à arma e devolve o que ela acertou.
    ///
    /// O pool fica **privado** e é esta função que o empresta à arma. Expor
    /// `&mut EnemyPool` faria qualquer chamador poder matar um cubo sem passar
    /// pela arma — e o abate do jogador e a chegada ao asteroide deixariam de ser
    /// distinguíveis, que é justamente o que [`Partida::derrotas`] mede.
    pub fn atirar(&mut self, arma: &mut Weapon, origem: Vec3, direcao: Vec3) -> Option<Atingido> {
        if self.game_over {
            return None;
        }
        arma.atirar(origem, direcao, &mut self.pool)
    }

    /// Dispara e devolve **todos** os cubos abatidos.
    ///
    /// O pulso devolve no máximo um; o leque até três; o morteiro leva o que
    /// estiver perto do impacto. A partida só empresta o pool: quem decide o que
    /// a arma faz é a arma.
    pub fn disparar(&mut self, arma: &mut Weapon, origem: Vec3, direcao: Vec3) -> Vec<Atingido> {
        if self.game_over {
            return Vec::new();
        }
        arma.disparar(origem, direcao, &mut self.pool)
    }

    /// Reinício de partida.
    pub fn limpar(&mut self) {
        self.pool.limpar();
        self.desde_ultimo_spawn = 0.0;
        self.game_over_avisado = false;
        self.game_over = false;
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
        let shader = render.create_builtin_shader(BuiltinShader::Phong)?;
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
        // Metal quente: especular mais forte que a rocha, para os cubos lerem
        // como ameaça e não como pedra. `set_light` é aplicado **por desenho**
        // (o `bind_and_upload` reenvia os uniformes), então trocar aqui não
        // afeta o asteroide desenhado antes.
        render.set_light(crate::gameplay::arena::luz_dos_inimigos());
        for (_, inimigo) in partida.pool().iter_alive() {
            render.draw_mesh_colored(
                self.malha,
                self.shader,
                &Transform3D::from_translation(inimigo.position),
                camera,
                inimigo.cor,
            )?;
        }
        render.set_light(crate::gameplay::arena::luz_do_asteroide());
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
    fn o_tiro_mata_e_nao_conta_como_derrota() {
        // A distinção que o jogo inteiro depende: cubo que chega ao asteroide é
        // dano; cubo abatido é ponto. Se `atirar` mexesse em `derrotas`, o HUD do
        // 2b mostraria "inimigos perdidos" crescendo quando o jogador acerta.
        let (mut partida, estado) = partida();
        partida.povoar(1);
        assert_eq!(partida.inimigos(), 1);

        let mut arma = Weapon::new();
        // Mira no cubo 0: nasce entre 15 e 20 unidades, então o tiro do centro
        // na direção dele acerta.
        let (_, alvo) = partida.pool().iter_alive().next().expect("um cubo");
        let direcao = alvo.direcao_ao_alvo(Vec3::ZERO);
        let origem = alvo.position;
        let tiro = partida.atirar(&mut arma, origem, direcao);
        assert!(tiro.is_some(), "o tiro devia acertar o cubo");
        assert_eq!(partida.inimigos(), 0, "o cubo abatido sai do pool");
        assert_eq!(partida.derrotas(), 0, "abate não é derrota");
        assert_eq!(estado.lives, VIDAS_INICIAIS, "abate não tira vida");
    }

    #[test]
    fn o_cubo_plantado_fica_parado() {
        // O `--alvo` depende disto: se ele se movesse, o terceiro tiro erraria e
        // o diagnóstico culparia a arma.
        let (mut partida, mut estado) = partida();
        let posicao = Vec3::new(0.0, 0.0, 9.0);
        assert!(partida.plantar(posicao, 0.5).is_some());
        partida.atualizar(1.0, &mut estado);
        let (_, cubo) = partida.pool().iter_alive().next().expect("um cubo");
        assert_eq!(cubo.position, posicao, "o cubo plantado se moveu");
    }

    #[test]
    fn nao_se_atira_depois_do_fim() {
        // `game_over` só é marcado pelo caminho real (cubo chegando ao
        // asteroide). Escrever `estado.lives = 0` de fora **não** marca — e é por
        // isso que este teste deixa a partida perder sozinha: um teste que
        // forjasse o estado provaria só que o campo existe.
        let (mut partida, mut estado) = partida();
        // Cinco cubos nascendo a cada 2 s e levando 5 a 6,7 s para chegar: a
        // última vida cai perto dos 15 s. 40 s é folga, e o laço sai antes.
        let mut segundos = 0.0;
        while segundos < 40.0 && !estado.acabou() {
            partida.atualizar(PASSO_MAX, &mut estado);
            segundos += PASSO_MAX;
        }
        assert!(estado.acabou(), "a partida devia ter acabado em 40 s");

        partida.povoar(1);
        let antes = partida.inimigos();
        assert!(antes > 0);
        let mut arma = Weapon::new();
        let (_, alvo) = partida.pool().iter_alive().next().expect("um cubo");
        let tiro = partida.atirar(&mut arma, alvo.position, alvo.direcao_ao_alvo(Vec3::ZERO));
        assert!(tiro.is_none(), "depois do GAME OVER o tiro não pode matar");
        assert_eq!(
            partida.inimigos(),
            antes,
            "nenhum cubo pode ter morrido depois do GAME OVER"
        );
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

    #[test]
    fn a_arma_de_area_abate_varios_numa_chamada() {
        // O caminho do jogo é `disparar`, não `atirar`: com o `atirar`, dois dos
        // três cubos do leque morriam **sem ponto e sem som** — o pool os
        // removia e o placar não via. Medido no aparelho antes da correção.
        let (mut partida, _estado) = partida();
        let mut postos = 0;
        // Negativo: a câmera inicial olha de `+Z` para `-Z`, então o que está à
        // frente tem `z` menor que o da origem.
        for z in [-8.0f32, -9.0, -10.0] {
            if partida.plantar(Vec3::new(0.0, 0.0, z), 0.5).is_some() {
                postos += 1;
            }
        }
        assert_eq!(postos, 3);
        let mut leque = Weapon::com_arma(&crate::gameplay::weapon::LEQUE);
        let atingidos = partida.disparar(&mut leque, Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0));
        assert_eq!(atingidos.len(), 3, "o leque devia abater os três");
        assert_eq!(partida.inimigos(), 0);
        assert_eq!(partida.derrotas(), 0, "abate não é derrota");
    }

    #[test]
    fn o_pulso_continua_abatendo_um_por_disparo() {
        let (mut partida, _estado) = partida();
        for z in [-8.0f32, -9.0, -10.0] {
            partida.plantar(Vec3::new(0.0, 0.0, z), 0.5);
        }
        let mut pulso = Weapon::new();
        let atingidos = partida.disparar(&mut pulso, Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0));
        assert_eq!(atingidos.len(), 1, "o pulso é de um alvo");
        assert_eq!(partida.inimigos(), 2);
    }

    #[test]
    fn o_campo_do_modo_facil_afasta_o_spawn_da_camera() {
        // O defeito que isto impede: a câmera do modo fácil fica a 18 e a casca
        // fixa do 1b ia até 20 — metade dos cubos nasceria atrás do jogador.
        for modo in crate::gameplay::modo::Modo::todos() {
            let partida = Partida::com_modo(1, modo);
            let campo = partida.campo();
            assert!(
                campo.raio_min > modo.distancia_da_camera() + 5.0,
                "{}: casca em {:.1} com câmera a {:.1}",
                modo.nome(),
                campo.raio_min,
                modo.distancia_da_camera()
            );
        }
        // E o difícil é exatamente o campo do bloco 1b.
        let dificil = Partida::com_modo(1, crate::gameplay::modo::Modo::Dificil);
        assert_eq!(dificil.campo(), crate::gameplay::campo::Campo::padrao());
    }

    #[test]
    fn o_campo_acompanha_o_zoom_e_nao_so_o_modo() {
        // O jogador pode dar zoom: no modo difícil o teto é 15, e a casca do
        // difícil é `15..20`. Com o olho em 15 e a casca em 15, o cubo nasceria
        // **em cima da câmera**. O laço chama `definir_campo` com a distância
        // real; este teste fixa a conta que ele usa.
        use crate::gameplay::campo::Campo;
        use crate::gameplay::modo::Modo;

        let mut partida = Partida::com_modo(1, Modo::Dificil);
        assert_eq!(partida.campo(), Campo::padrao());

        for distancia in [5.0f32, 9.0, 12.0, 15.0, 18.0] {
            partida.definir_campo(Campo::para_distancia(distancia));
            assert!(
                partida.campo().raio_min > distancia,
                "câmera a {distancia}: casca em {:.1}",
                partida.campo().raio_min
            );
        }
        // E o modo não foi tocado: quem manda no campo agora é a câmera.
        assert_eq!(partida.modo(), Modo::Dificil);
    }
}
