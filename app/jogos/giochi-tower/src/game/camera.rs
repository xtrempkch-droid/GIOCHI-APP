//! Mapa, câmera isométrica e o raio de interesse (AOI).
//!
//! # Por que a câmera não é ortográfica
//!
//! O GDD §6 pede "top-down isométrica". A engine **não tem câmera ortográfica**:
//! `Camera3D` só monta `Mat4::perspective` (`math3d/camera.rs:118`), e
//! `draw_mesh*` só aceita `&Camera3D` — `Mat4::orthographic` existe, mas é
//! inalcançável pelo caminho de desenho. A câmera aqui é, portanto, uma
//! perspectiva com o ângulo isométrico clássico (yaw 45°, pitch atan(1/√2) ≈
//! 35,264°) e FOV estreito. Ver DECISIONS.md (D-003).
//!
//! # AOI
//!
//! O GDD §6 pede raio "~1,5 × meia-diagonal da tela". Como não há tela plana — há
//! um frustum projetado no chão — o raio é derivado do frustum: meia-altura no
//! plano do foco é `d·tan(fov/2)`; no chão, a profundidade é esticada por
//! `1/sin(pitch)`. A meia-diagonal do quadrilátero resultante É a "meia-diagonal
//! da tela" do GDD.

use glam::Vec3;
use giochi_engine_3d::math3d::{Camera3D, Vec3 as EngineVec3};

/// Uma "tela" do mapa em metros. O GDD fala em 4×4 telas, não em metros: esta
/// constante é a decisão que falta no documento (DECISIONS.md, D-004).
pub const SCREEN_SIZE_M: f32 = 40.0;

/// Quantas telas por eixo (GDD §6: 4×4 = 16 telas).
pub const SCREENS_PER_AXIS: u32 = 4;

/// Yaw padrão, em graus: 45° dá a diagonal isométrica.
pub const DEFAULT_YAW_DEG: f32 = 45.0;

/// Pitch padrão, em graus: `atan(1/√2)` — o ângulo isométrico clássico.
pub const DEFAULT_PITCH_DEG: f32 = 35.264_39;

/// Distância da câmera ao foco, em metros.
pub const DEFAULT_DISTANCE_M: f32 = 52.0;

/// FOV vertical, em graus. Estreito de propósito: aproxima a projeção
/// isométrica sem perder a profundidade que a engine só sabe fazer.
pub const DEFAULT_FOV_Y_DEG: f32 = 32.0;

/// Plano próximo, em metros.
pub const DEFAULT_NEAR_M: f32 = 0.5;

/// Plano distante, em metros.
///
/// Cobre o chão de 600 m (`game::cenario::LADO_DO_CHAO_M`) **a partir do olho**:
/// o canto mais distante do plano fica a ~470 m da câmera, e o valor antigo
/// (420) cortava esse canto numa linha reta — o "horizonte" era o plano de corte,
/// não a paisagem. 900 m deixa folga para as mesas e mantém precisão de
/// profundidade suficiente (24 bits sobre 0,5..900 m dá ~1 cm perto da câmera).
pub const DEFAULT_FAR_M: f32 = 900.0;

/// Limites do campo de batalha, no plano XZ.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapBounds {
    /// Meia-extensão em X e em Z; o mapa é quadrado e centrado na origem.
    pub half_extent_m: f32,
}

impl MapBounds {
    /// O mapa do GDD: 4×4 telas de [`SCREEN_SIZE_M`].
    pub const fn new() -> Self {
        Self {
            half_extent_m: SCREEN_SIZE_M * SCREENS_PER_AXIS as f32 * 0.5,
        }
    }

    /// Lado do mapa, em metros.
    pub fn size_m(self) -> f32 {
        self.half_extent_m * 2.0
    }

    /// Traz um ponto para dentro do mapa, preservando Y.
    pub fn clamp(self, point: Vec3) -> Vec3 {
        Vec3::new(
            point.x.clamp(-self.half_extent_m, self.half_extent_m),
            point.y,
            point.z.clamp(-self.half_extent_m, self.half_extent_m),
        )
    }

    /// `true` se o ponto está dentro do mapa (ignora Y).
    pub fn contains(self, point: Vec3) -> bool {
        point.x.abs() <= self.half_extent_m && point.z.abs() <= self.half_extent_m
    }
}

impl Default for MapBounds {
    fn default() -> Self {
        Self::new()
    }
}

/// Câmera top-down isométrica, uma por jogador (GDD §6).
///
/// O estado é o *foco* no chão, não a posição do olho: mover a câmera é mover o
/// ponto que o jogador está olhando, e o olho é derivado. Isso evita a
/// dessincronização clássica entre "onde olho" e "para onde ando".
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IsoCamera {
    /// Ponto olhado, no plano do chão (Y é a altura do foco).
    pub focus: Vec3,
    /// Giro em torno de Y, em radianos.
    pub yaw_rad: f32,
    /// Inclinação acima do horizonte, em radianos.
    pub pitch_rad: f32,
    /// Distância do olho ao foco, em metros.
    pub distance_m: f32,
    /// FOV vertical, em radianos.
    pub fov_y_rad: f32,
    /// Plano próximo, em metros.
    pub near_m: f32,
    /// Plano distante, em metros.
    pub far_m: f32,
}

impl IsoCamera {
    /// Câmera nos ângulos padrão, olhando `focus`.
    pub fn new(focus: Vec3) -> Self {
        Self {
            focus,
            yaw_rad: DEFAULT_YAW_DEG.to_radians(),
            pitch_rad: DEFAULT_PITCH_DEG.to_radians(),
            distance_m: DEFAULT_DISTANCE_M,
            fov_y_rad: DEFAULT_FOV_Y_DEG.to_radians(),
            near_m: DEFAULT_NEAR_M,
            far_m: DEFAULT_FAR_M,
        }
    }

    /// Posição do olho: o foco recuado pela direção de visão.
    ///
    /// A direção segue a convenção da engine (`cube_demo.rs:167`): +Y é para
    /// cima e o plano do chão é XZ.
    pub fn eye(&self) -> Vec3 {
        let cos_pitch = self.pitch_rad.cos();
        let offset = Vec3::new(
            cos_pitch * self.yaw_rad.sin(),
            self.pitch_rad.sin(),
            cos_pitch * self.yaw_rad.cos(),
        );
        self.focus + offset * self.distance_m
    }

    /// Direção "para frente" projetada no chão (de onde o olho olha).
    pub fn forward_ground(&self) -> Vec3 {
        Vec3::new(-self.yaw_rad.sin(), 0.0, -self.yaw_rad.cos())
    }

    /// Direção "para a direita" projetada no chão.
    pub fn right_ground(&self) -> Vec3 {
        Vec3::new(self.yaw_rad.cos(), 0.0, -self.yaw_rad.sin())
    }

    /// Converte um stick/teclado (x = direita, y = frente) em direção de mundo
    /// no chão. Já normalizado e limitado a 1.
    pub fn ground_direction(&self, x: f32, y: f32) -> Vec3 {
        let direction = self.right_ground() * x + self.forward_ground() * y;
        let length = direction.length();
        if length <= 1.0e-6 {
            return Vec3::ZERO;
        }
        // Limita a 1 sem normalizar o vetor curto em zero: o analógico devolve
        // magnitudes intermediárias e o jogador espera andar mais devagar.
        direction / length.max(1.0)
    }

    /// Move o foco no plano do chão, relativo à câmera.
    pub fn pan(&mut self, forward: f32, right: f32, speed_mps: f32, dt: f32) {
        let delta = self.ground_direction(right, forward) * speed_mps * dt;
        self.focus += delta;
        self.focus.y = 0.0;
    }

    /// Traz o foco para dentro do mapa.
    pub fn clamp_focus(&mut self, map: MapBounds) {
        self.focus = map.clamp(self.focus);
    }

    /// Meia-largura e meia-profundidade (no chão) do que a câmera enxerga no
    /// plano do foco, em metros.
    pub fn ground_half_extent(&self, aspect: f32) -> (f32, f32) {
        let half_height = self.distance_m * (self.fov_y_rad * 0.5).tan();
        let half_width = half_height * aspect.max(0.01);
        // A profundidade no chão é a altura do frustum "esticada" pela
        // inclinação: com a câmera rasante o alcance no chão explode, e com a
        // câmera a pino ele volta a ser half_height.
        let sin_pitch = self.pitch_rad.sin().max(0.05);
        (half_width, half_height / sin_pitch)
    }

    /// Raio de AOI do GDD §6: `aoi_scale` × meia-diagonal do que se vê.
    ///
    /// `aoi_scale` vem do perfil de host (1,5 nos perfis normais, 1,0 no
    /// RK3326, que é "AOI agressiva" — menos inimigos por cliente).
    pub fn aoi_radius(&self, aspect: f32, aoi_scale: f32) -> f32 {
        let (half_width, half_depth) = self.ground_half_extent(aspect);
        (half_width * half_width + half_depth * half_depth).sqrt() * aoi_scale.max(0.0)
    }

    /// Câmera no formato que a engine consome.
    ///
    /// O `aspect` é resolvido contra o drawable real (`with_drawable_size`) —
    /// no KMSDRM o painel não é o tamanho pedido (ADR-035, `cube_demo.rs:165`).
    pub fn engine_camera(&self, drawable_width: u32, drawable_height: u32) -> Camera3D {
        Camera3D::from_parts(
            EngineVec3::from_glam(self.eye()),
            EngineVec3::from_glam(self.focus),
            EngineVec3::UP,
        )
        .with_fov_y(self.fov_y_rad)
        .with_clip_planes(self.near_m, self.far_m)
        .with_drawable_size(drawable_width.max(1), drawable_height.max(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn camera() -> IsoCamera {
        IsoCamera::new(Vec3::ZERO)
    }

    #[test]
    fn the_eye_is_the_configured_distance_from_the_focus() {
        let c = camera();
        let distance = (c.eye() - c.focus).length();
        assert!(
            (distance - DEFAULT_DISTANCE_M).abs() < 1.0e-3,
            "olho a {distance} m, esperava {DEFAULT_DISTANCE_M}"
        );
        assert!(c.eye().y > c.focus.y, "a câmera olha de cima");
    }

    #[test]
    fn a_ninety_degree_pitch_looks_straight_down() {
        let mut c = camera();
        c.pitch_rad = std::f32::consts::FRAC_PI_2;
        let offset = c.eye() - c.focus;
        assert!(offset.x.abs() < 1.0e-5 && offset.z.abs() < 1.0e-5, "sem deslocamento lateral");
        assert!((offset.y - DEFAULT_DISTANCE_M).abs() < 1.0e-3);
    }

    #[test]
    fn forward_and_right_are_horizontal_and_orthogonal() {
        let c = camera();
        let forward = c.forward_ground();
        let right = c.right_ground();
        assert!(forward.y.abs() < 1.0e-6 && right.y.abs() < 1.0e-6);
        assert!(forward.dot(right).abs() < 1.0e-6, "eixos do chão ortogonais");
        assert!((forward.length() - 1.0).abs() < 1.0e-5);
        assert!((right.length() - 1.0).abs() < 1.0e-5);
    }

    #[test]
    fn panning_forward_moves_towards_the_view() {
        let mut c = camera();
        let before = c.forward_ground();
        c.pan(1.0, 0.0, 10.0, 0.5);
        let moved = c.focus;
        assert!(moved.length() > 4.9, "andou 5 m em 0,5 s a 10 m/s: {moved:?}");
        assert!(moved.normalize().dot(before) > 0.99, "andou na direção da visão");
        assert_eq!(c.focus.y, 0.0, "o foco nunca sai do chão");
    }

    #[test]
    fn the_analog_is_not_renormalised_to_full_speed() {
        let c = camera();
        let half = c.ground_direction(0.0, 0.5);
        let full = c.ground_direction(0.0, 1.0);
        assert!((half.length() - 0.5).abs() < 1.0e-5, "meio analógico = meia velocidade");
        assert!((full.length() - 1.0).abs() < 1.0e-5);
    }

    #[test]
    fn aoi_grows_with_the_aspect_and_with_the_distance() {
        let c = camera();
        let square = c.aoi_radius(1.0, 1.5);
        let wide = c.aoi_radius(16.0 / 9.0, 1.5);
        assert!(wide > square, "tela mais larga vê mais chão");

        let mut far = camera();
        far.distance_m *= 2.0;
        assert!(far.aoi_radius(1.0, 1.5) > square * 1.9, "frustum maior, AOI maior");
    }

    #[test]
    fn the_rk3326_profile_filters_harder_than_the_balanced_one() {
        let c = camera();
        assert!(c.aoi_radius(16.0 / 9.0, 1.0) < c.aoi_radius(16.0 / 9.0, 1.5));
    }

    #[test]
    fn the_map_is_four_by_four_screens() {
        let map = MapBounds::new();
        assert_eq!(map.size_m(), 160.0);
        assert_eq!(map.half_extent_m, 80.0);
        assert!(map.contains(Vec3::new(79.0, 0.0, -79.0)));
        assert!(!map.contains(Vec3::new(81.0, 0.0, 0.0)));
    }

    #[test]
    fn clamping_brings_a_stray_focus_back_inside() {
        let mut c = IsoCamera::new(Vec3::new(500.0, 0.0, -500.0));
        c.clamp_focus(MapBounds::new());
        assert_eq!(c.focus, Vec3::new(80.0, 0.0, -80.0));
    }

    #[test]
    fn the_engine_camera_keeps_the_focus_as_target() {
        let c = camera();
        let engine = c.engine_camera(1280, 720);
        assert_eq!(engine.target.to_array(), [0.0, 0.0, 0.0]);
        assert!((engine.aspect - 1280.0 / 720.0).abs() < 1.0e-5);
    }
}
