//! Câmera orbital em volta da arena.
//!
//! O jogador não anda: ele gira em torno do asteroide central, que é o que um
//! tower defense espacial pede. Três números descrevem tudo — `yaw`, `pitch` e
//! `distance` — e todos os três têm limite, porque um `pitch` de 90° deixa o
//! vetor "para cima" paralelo à direção de visão e a matriz de visão deixa de
//! existir (o `look_at` da engine devolve erro, e o quadro sai preto).
//!
//! A conversão para [`Camera3D`] acontece a cada quadro e resolve o aspecto com o
//! tamanho **real** do drawable: no aparelho o painel não é o 1280×720 pedido
//! (ADR-035 da engine), e uma câmera que fixasse 16:9 renderizaria uma cena
//! cortada sem nenhum erro.

use crate::utils::math::{clamp, graus_para_radianos, normalizar_angulo};
use giochi_engine_3d::math3d::{Camera3D, Vec3};

/// Distância mínima ao alvo, em unidades de mundo.
pub const DISTANCIA_MINIMA: f32 = 5.0;
/// Distância máxima ao alvo, em unidades de mundo.
pub const DISTANCIA_MAXIMA: f32 = 15.0;
/// Inclinação mínima: 60° abaixo do plano do horizonte.
pub const PITCH_MINIMO: f32 = -std::f32::consts::FRAC_PI_3;
/// Inclinação máxima: 80° acima do plano do horizonte.
///
/// Não são 90° de propósito: a 90° o vetor `UP` fica paralelo à direção de visão
/// e o `look_at` não tem solução. 80° já é "quase de cima" e mantém a matriz
/// válida. O valor é `4 * PI / 9`, que é 80° em radianos.
pub const PITCH_MAXIMO: f32 = 4.0 * std::f32::consts::PI / 9.0;

/// Radianos de giro por pixel de mouse.
///
/// 0.0045 ≈ 0.26° por pixel: uma travessia de 1280 px gira ~330°. Mais que isso
/// fica impossível de mirar; menos, e o jogador precisa de três gestos para
/// olhar para trás.
pub const SENSIBILIDADE: f32 = 0.0045;

/// Fator aplicado a cada "clique" de roda.
pub const PASSO_ZOOM: f32 = 1.0;

/// Câmera orbital: ângulos e distância em volta de um alvo fixo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitalCamera {
    /// Giro em torno do eixo Y, em radianos.
    pub yaw: f32,
    /// Inclinação em relação ao plano horizontal, em radianos.
    pub pitch: f32,
    /// Distância até o alvo, em unidades de mundo.
    pub distance: f32,
    /// Teto de afastamento, em unidades.
    ///
    /// O modo fácil pede a câmera ao **dobro** da distância (18), e o teto fixo
    /// de 15 do bloco 1a a prenderia em 15 — o modo não faria o que promete e
    /// ninguém veria erro nenhum. O limite virou campo para o modo poder estendê-lo
    /// sem afrouxar o do modo difícil.
    pub distancia_maxima: f32,
    /// Ponto observado — o centro do asteroide.
    pub target: Vec3,
}

impl OrbitalCamera {
    /// Câmera inicial: um pouco acima do plano, a 9 unidades, olhando de frente.
    ///
    /// `yaw = 0` põe a câmera em `+Z` olhando para `-Z`, que é a convenção da
    /// engine (`Camera3D::new` olha para `-Z`).
    pub fn new() -> Self {
        Self {
            yaw: 0.0,
            pitch: graus_para_radianos(22.0),
            distance: crate::gameplay::modo::DISTANCIA_DE_REFERENCIA,
            distancia_maxima: DISTANCIA_MAXIMA,
            target: Vec3::ZERO,
        }
    }

    /// Câmera com a distância e o teto de um modo de dificuldade.
    ///
    /// O teto é `máximo(distância do modo, DISTANCIA_MAXIMA)`: o modo fácil
    /// permite afastar até 18, o difícil continua preso em 15 — mas nenhum dos
    /// dois impede o jogador de **aproximar** se quiser.
    pub fn com_modo(modo: crate::gameplay::modo::Modo) -> Self {
        let distancia = modo.distancia_da_camera();
        Self {
            distance: distancia,
            distancia_maxima: distancia.max(DISTANCIA_MAXIMA),
            ..Self::new()
        }
    }

    /// Câmera com um alvo explícito, para quando a arena sair da origem.
    pub fn com_alvo(alvo: Vec3) -> Self {
        Self {
            target: alvo,
            ..Self::new()
        }
    }

    /// Posição do olho, calculada a partir dos três parâmetros.
    ///
    /// `pitch` positivo sobe; `yaw` gira no sentido anti-horário visto de cima.
    pub fn olho(&self) -> Vec3 {
        let cos_pitch = self.pitch.cos();
        Vec3::new(
            self.target.x + self.distance * cos_pitch * self.yaw.sin(),
            self.target.y + self.distance * self.pitch.sin(),
            self.target.z + self.distance * cos_pitch * self.yaw.cos(),
        )
    }

    /// Gira a câmera por um deslocamento de mouse, em pixels.
    ///
    /// `dy` é positivo para baixo, como o SDL entrega. Por isso o sinal é
    /// subtraído: puxar o mouse para cima sobe a câmera, que é o que a mão espera.
    pub fn orbitar(&mut self, dx: f32, dy: f32) {
        self.girar(dx * SENSIBILIDADE, -dy * SENSIBILIDADE);
    }

    /// Gira por ângulos já em radianos.
    ///
    /// É o caminho de quem não tem mouse: teclado e analógico medem a taxa em
    /// radianos por segundo e multiplicam pelo passo fixo. Uma segunda conversão
    /// em pixels seria uma segunda constante para manter em sincronia.
    pub fn girar(&mut self, delta_yaw: f32, delta_pitch: f32) {
        if !delta_yaw.is_finite() || !delta_pitch.is_finite() {
            return;
        }
        self.yaw = normalizar_angulo(self.yaw + delta_yaw);
        self.pitch = clamp(self.pitch + delta_pitch, PITCH_MINIMO, PITCH_MAXIMO);
    }

    /// Aproxima (`passos` positivo) ou afasta, preso entre os dois limites.
    pub fn zoom(&mut self, passos: f32) {
        if !passos.is_finite() {
            return;
        }
        self.distance = clamp(
            self.distance - passos * PASSO_ZOOM,
            DISTANCIA_MINIMA,
            self.teto(),
        );
    }

    /// Distância já presa ao intervalo válido.
    ///
    /// O campo é público — o `--teclas` e os testes escrevem nele direto —, então
    /// a validação acontece na leitura, não só na escrita.
    pub fn distancia_segura(&self) -> f32 {
        clamp(self.distance, DISTANCIA_MINIMA, self.teto())
    }

    /// Teto de afastamento, já saneado.
    ///
    /// O campo é público e o `--distancia` escreve nele; um teto `NaN` faria o
    /// `clamp` devolver `NaN` e a matriz de visão deixaria de existir — quadro
    /// preto sem erro.
    pub fn teto(&self) -> f32 {
        if self.distancia_maxima.is_finite() && self.distancia_maxima >= DISTANCIA_MINIMA {
            self.distancia_maxima
        } else {
            DISTANCIA_MAXIMA
        }
    }

    /// Inclinação já presa ao intervalo válido.
    pub fn pitch_seguro(&self) -> f32 {
        clamp(self.pitch, PITCH_MINIMO, PITCH_MAXIMO)
    }

    /// Monta a [`Camera3D`] da engine para o tamanho real do drawable.
    ///
    /// Passar `0, 0` deixa o aspecto em `0.0`, que na engine significa "pergunta
    /// ao renderer" — é o caminho para uma captura headless, onde não há janela.
    pub fn camera(&self, largura: u32, altura: u32) -> Camera3D {
        self.camera_com_tremor(largura, altura, (0.0, 0.0))
    }

    /// Câmera com uma translação de tremor somada ao olho **e** ao alvo.
    ///
    /// Transladar os dois juntos é o que faz o quadro inteiro deslizar sem girar:
    /// a direção de visão não muda, o enquadramento não muda, e o asteroide sai
    /// do lugar junto com o fundo. Girar `yaw`/`pitch` em volta do alvo (a
    /// primeira versão) mantém o asteroide **centrado** e o efeito quase não
    /// aparece — medido: 1,14 px de deslocamento em 720p.
    ///
    /// O deslocamento entra numa **cópia**, não no estado: o tremor não acumula,
    /// então dez danos seguidos não deixam a câmera deslocada quando ele acaba.
    /// E `distance` não muda — tremer aproximando e afastando pareceria zoom.
    pub fn camera_com_tremor(
        &self,
        largura: u32,
        altura: u32,
        deslocamento: (f32, f32),
    ) -> Camera3D {
        let (desloc_x, desloc_y) = deslocamento;
        let desloc_x = if desloc_x.is_finite() { desloc_x } else { 0.0 };
        let desloc_y = if desloc_y.is_finite() { desloc_y } else { 0.0 };
        let distancia = self.distancia_segura();
        let pitch = self.pitch_seguro();
        let yaw = normalizar_angulo(self.yaw);
        let cos_pitch = pitch.cos();
        let olho = Vec3::new(
            self.target.x + distancia * cos_pitch * yaw.sin(),
            self.target.y + distancia * pitch.sin(),
            self.target.z + distancia * cos_pitch * yaw.cos(),
        );

        // Deslocamento perpendicular à linha de visão: `direita` no eixo X do
        // mundo e `UP` no Y. O eixo Z do mundo fica de fora porque, com a câmera
        // olhando para o alvo, deslocar em Z aproxima em vez de sacudir.
        let direita = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
        let alvo = Vec3::new(
            self.target.x + direita.x * desloc_x,
            self.target.y + desloc_y,
            self.target.z + direita.z * desloc_x,
        );
        let olho = Vec3::new(
            olho.x + direita.x * desloc_x,
            olho.y + desloc_y,
            olho.z + direita.z * desloc_x,
        );

        let camera = Camera3D::from_parts(olho, alvo, Vec3::UP)
            // O céu das estrelas fica a ~120 unidades; o padrão de 1000 é folgado
            // e não custa precisão de profundidade em 16 bits, mas 400 deixa
            // margem para o asteroide e para os inimigos próximos.
            .with_clip_planes(0.1, 400.0);

        if largura > 0 && altura > 0 {
            camera.with_drawable_size(largura, altura)
        } else {
            camera
        }
    }
}

impl Default for OrbitalCamera {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::math::radianos_para_graus;

    #[test]
    fn a_camera_inicial_e_valida_e_olha_para_o_centro() {
        let camera = OrbitalCamera::new();
        let olho = camera.olho();
        let distancia = (olho.x * olho.x + olho.y * olho.y + olho.z * olho.z).sqrt();
        assert!(
            (distancia - camera.distance).abs() < 1e-4,
            "olho a {distancia}, distância configurada {}",
            camera.distance
        );
        assert!(camera.camera(1280, 720).view_matrix().is_ok());
    }

    #[test]
    fn o_zoom_para_nos_dois_limites() {
        let mut camera = OrbitalCamera::new();
        camera.zoom(1000.0);
        assert_eq!(camera.distance, DISTANCIA_MINIMA);
        camera.zoom(-1000.0);
        assert_eq!(camera.distance, DISTANCIA_MAXIMA);
    }

    #[test]
    fn o_pitch_nao_chega_a_noventa_graus() {
        // A 90° o `look_at` não tem solução. Se o limite for afrouxado, este
        // teste falha antes de a tela ficar preta no aparelho.
        let mut camera = OrbitalCamera::new();
        camera.orbitar(0.0, -100_000.0);
        assert!(camera.pitch <= PITCH_MAXIMO + 1e-6);
        assert!(radianos_para_graus(camera.pitch) < 90.0);
        assert!(
            camera.camera(1280, 720).view_matrix().is_ok(),
            "a matriz de visão deixou de existir no limite superior de pitch"
        );

        camera.orbitar(0.0, 100_000.0);
        assert!(camera.pitch >= PITCH_MINIMO - 1e-6);
        assert!(radianos_para_graus(camera.pitch) > -90.0);
        assert!(camera.camera(1280, 720).view_matrix().is_ok());
    }

    #[test]
    fn o_yaw_fica_sempre_pequeno() {
        let mut camera = OrbitalCamera::new();
        for _ in 0..1000 {
            camera.orbitar(97.0, 0.0);
        }
        assert!(
            camera.yaw > -std::f32::consts::PI - 1e-4 && camera.yaw <= std::f32::consts::PI + 1e-4,
            "yaw acumulou {} radianos",
            camera.yaw
        );
    }

    #[test]
    fn girar_meia_volta_troca_o_lado_do_olho() {
        let mut camera = OrbitalCamera::new();
        let antes = camera.olho();
        camera.orbitar(std::f32::consts::PI / SENSIBILIDADE, 0.0);
        let depois = camera.olho();
        assert!(
            (antes.x + depois.x).abs() < 1e-3 && (antes.z + depois.z).abs() < 1e-3,
            "meia volta devia espelhar o olho: {antes:?} → {depois:?}"
        );
    }

    #[test]
    fn o_mouse_para_cima_sobe_a_camera() {
        let mut camera = OrbitalCamera::new();
        let antes = camera.olho().y;
        camera.orbitar(0.0, -50.0);
        assert!(camera.olho().y > antes);
    }

    #[test]
    fn a_camera_ignora_entrada_nao_finita() {
        let mut camera = OrbitalCamera::new();
        let original = camera;
        camera.orbitar(f32::NAN, 10.0);
        camera.orbitar(10.0, f32::INFINITY);
        camera.zoom(f32::NAN);
        assert_eq!(camera, original);
    }

    #[test]
    fn campo_publico_fora_da_faixa_e_preso_na_leitura() {
        // O `--teclas` e os testes escrevem nos campos; a leitura é quem garante
        // o contrato. Sem isto, `distance = 0.0` produziria `position == target`.
        let camera = OrbitalCamera {
            distance: 0.0,
            pitch: 3.0,
            ..OrbitalCamera::new()
        };
        assert_eq!(camera.distancia_segura(), DISTANCIA_MINIMA);
        assert!(camera.camera(640, 480).view_matrix().is_ok());
    }

    /// Distância entre dois pontos do mundo.
    ///
    /// `Vec3` da engine não implementa `Sub` nem tem `distance` (o `glam` por
    /// baixo tem, mas a fachada expõe só `length`/`normalized`) — e converter
    /// para `glam` num teste de câmera é ruído.
    fn distancia(a: Vec3, b: Vec3) -> f32 {
        let (dx, dy, dz) = (a.x - b.x, a.y - b.y, a.z - b.z);
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    #[test]
    fn o_tremor_mexe_a_camera_e_nao_o_estado() {
        // O tremor entra numa **cópia**: `yaw`/`pitch` do estado não mudam, então
        // dez danos seguidos não deixam a câmera torta quando o tremor acaba.
        let base = OrbitalCamera::new();
        let tremida = base.camera_com_tremor(640, 480, (0.2, -0.1));
        assert_eq!(base, OrbitalCamera::new(), "o estado foi alterado");
        assert!(
            distancia(tremida.position, base.camera(640, 480).position) > 1e-2,
            "o tremor não moveu a câmera"
        );
        // Olho e alvo andam juntos: a direção de visão não pode mudar, senão o
        // tremor viraria um giro (e o alvo ficaria centrado, sem efeito).
        let antes = base.camera(640, 480);
        let direcao = |c: &Camera3D| {
            let v = Vec3::new(
                c.target.x - c.position.x,
                c.target.y - c.position.y,
                c.target.z - c.position.z,
            );
            v.normalized()
        };
        let d_antes = direcao(&antes);
        let d_depois = direcao(&tremida);
        assert!(
            distancia(d_antes, d_depois) < 1e-5,
            "o tremor girou a câmera em vez de transladar"
        );
        // A distância **ao alvo** não muda (ela muda em relação à origem, porque
        // a translação move os dois): tremer aproximando seria zoom.
        let d_base = distancia(antes.position, antes.target);
        let d_tremida = distancia(tremida.position, tremida.target);
        assert!(
            (d_base - d_tremida).abs() < 1e-3,
            "a distância ao alvo mudou: {d_base:.4} → {d_tremida:.4}"
        );
    }

    #[test]
    fn o_tremor_nao_quebra_o_limite_de_pitch() {
        // A 80° o vetor UP fica paralelo à visão e o `look_at` não tem solução.
        // Um tremor somado **depois** do limite produziria exatamente a matriz
        // inválida que o limite existe para evitar.
        let mut camera = OrbitalCamera::new();
        for pitch in [PITCH_MINIMO, PITCH_MAXIMO] {
            camera.pitch = pitch;
            for deslocamento in [(0.22, 0.22), (-0.22, -0.22), (0.0, 0.22)] {
                let com_tremor = camera.camera_com_tremor(640, 480, deslocamento);
                assert!(
                    com_tremor.view_matrix().is_ok(),
                    "tremor {deslocamento:?} a {pitch} invalidadou a matriz de visão"
                );
            }
        }
    }

    #[test]
    fn tremor_com_valor_invalido_e_ignorado() {
        let base = OrbitalCamera::new();
        let referencia = base.camera(640, 480).position;
        for ruim in [
            (f32::NAN, 0.0),
            (0.0, f32::NAN),
            (f32::INFINITY, f32::NEG_INFINITY),
        ] {
            let camera = base.camera_com_tremor(640, 480, ruim);
            assert!(
                distancia(camera.position, referencia) < 1e-6,
                "tremor {ruim:?} mexeu na câmera"
            );
            assert!(camera.view_matrix().is_ok());
        }
    }

    #[test]
    fn sem_tamanho_o_aspecto_fica_com_o_renderer() {
        let camera = OrbitalCamera::new().camera(0, 0);
        assert_eq!(camera.aspect, 0.0);
    }

    #[test]
    fn o_modo_facil_consegue_afastar_ate_o_dobro() {
        // O defeito que este teste impede: com o teto fixo de 15, o modo fácil
        // pedia 18 e a câmera ficava presa — o modo não fazia o que promete.
        let facil = OrbitalCamera::com_modo(crate::gameplay::modo::Modo::Facil);
        assert_eq!(facil.distance, 18.0);
        assert_eq!(facil.distancia_segura(), 18.0);
        assert!(facil.teto() >= 18.0);

        let medio = OrbitalCamera::com_modo(crate::gameplay::modo::Modo::Medio);
        assert_eq!(medio.distancia_segura(), 15.75);

        let dificil = OrbitalCamera::com_modo(crate::gameplay::modo::Modo::Dificil);
        assert_eq!(dificil.distancia_segura(), 9.0);
        assert_eq!(dificil.teto(), DISTANCIA_MAXIMA);
    }

    #[test]
    fn nenhum_modo_impede_o_jogador_de_aproximar() {
        // Aproximar é escolha do jogador — e no modo fácil é justamente o que ele
        // faria se quisesse dificultar. O que o modo define é o **teto**.
        for modo in crate::gameplay::modo::Modo::todos() {
            let mut camera = OrbitalCamera::com_modo(modo);
            camera.zoom(1000.0);
            assert_eq!(camera.distance, DISTANCIA_MINIMA, "{}", modo.nome());
            camera.zoom(-1000.0);
            assert_eq!(
                camera.distance,
                camera.teto(),
                "{}: afastar para além do teto",
                modo.nome()
            );
        }
    }

    #[test]
    fn teto_invalido_nao_produz_nan() {
        // `--distancia` e testes escrevem no campo direto; um `NaN` ali faria a
        // matriz de visão deixar de existir e o quadro sairia preto sem erro.
        let mut camera = OrbitalCamera::new();
        camera.distancia_maxima = f32::NAN;
        assert!(camera.distancia_segura().is_finite());
        assert_eq!(camera.teto(), DISTANCIA_MAXIMA);
        camera.distancia_maxima = 1.0;
        assert_eq!(camera.teto(), DISTANCIA_MAXIMA, "teto abaixo do mínimo");
    }

    #[test]
    fn a_camera_de_qualquer_modo_olha_para_o_centro() {
        // Um teto novo não pode quebrar o `look_at`: a 18 unidades o olho ainda
        // tem de estar na direção certa.
        for modo in crate::gameplay::modo::Modo::todos() {
            let camera = OrbitalCamera::com_modo(modo);
            let olho = camera.olho();
            let distancia = (olho.x * olho.x + olho.y * olho.y + olho.z * olho.z).sqrt();
            assert!(
                (distancia - camera.distance).abs() < 1e-4,
                "{}: olho a {distancia}",
                modo.nome()
            );
            assert!(camera.camera(640, 480).view_matrix().is_ok());
        }
    }
}
