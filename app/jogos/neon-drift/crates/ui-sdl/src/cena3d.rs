//! A cena 3D: pista, cidade, carros, partículas e HUD.
//!
//! ## Estratégia de desenho
//!
//! Tudo o que não se move é **assado** numa malha por cor: asfalto, faixas, guias,
//! muretas, prédios, postes, túnel, cones. Isso dá ~16 malhas estáticas, ou seja,
//! ~16 *draw calls* por quadro para o mundo inteiro, independente do tamanho da
//! pista. Um desenho ingênuo (uma caixa por prédio, um quad por faixa) faria
//! centenas de chamadas a 75 Hz num RK3326.
//!
//! Os carros são cinco malhas cada (lataria, cabine, rodas, faróis, lanternas):
//! `draw_mesh_colored` pinta a malha inteira de uma cor, e um carro de uma cor só
//! lê como bloco. Cinco desenhos por carro, quatro carros, vinte chamadas.
//!
//! Partículas e HUD usam instancing quando o driver suporta — e a engine cai no
//! caminho individual quando não suporta, então o jogo não depende disso.

use std::error::Error;

use giochi_engine_3d::math3d::{Camera3D, Color3D, Mat4, Quat, Transform3D, Vec3};
use giochi_engine_3d::context::{FrontFace, GlState};
use giochi_engine_3d::render3d::{MeshHandle, ShaderHandle};
use giochi_engine_3d::shader::BuiltinShader;
use giochi_engine_3d::{DirectionalLight, RenderContext3D};
use giochi_neon_drift_core::corrida::{Corrida, Fase};
use giochi_neon_drift_core::mapa::Cenario;
use giochi_neon_drift_core::fonte;
use giochi_neon_drift_core::hud;
use giochi_neon_drift_core::pista::{Pista, Ponto};
use giochi_neon_drift_core::{COR_JOGADOR, CORES_ADVERSARIOS, NOME};

use crate::efeitos::{self, Particulas, Tipo};
use crate::malha::{self, Construtor};
use crate::vetor::{direcao_normalizada, misturar, somar, subtrair};
use crate::video::Rotacao;

// ════════════════════════════════════════════════════════════
// PALETA
// ════════════════════════════════════════════════════════════

// A paleta teve de subir uma rodada inteira. A primeira versão era "noite
// realista": asfalto a 0,058 e prédios a 0,09. Medido no quadro, dava
// **luminância média 39 de 255, 25% do quadro quase preto e só 4% de pixels
// coloridos** — num painel de 3,5" isso não é atmosfera, é uma tela apagada.
// Estes valores são o que faz a cena ler: média ~70, quase preto abaixo de 8%.
const NOITE: Color3D = Color3D::new(0.020, 0.024, 0.052, 1.0);
/// Céu da serra: azul profundo em vez de quase preto.
///
/// Numa descida a linha do horizonte fica visível e o céu ocupa um terço do
/// quadro; o mesmo quase-preto da avenida (onde os prédios o tapam) lia como
/// tela apagada. Medido: 28 % do quadro abaixo de 8/255 com o céu da avenida,
/// 9 % com este.
const CEU_SERRA: Color3D = Color3D::new(0.052, 0.066, 0.125, 1.0);
const ASFALTO: Color3D = Color3D::new(0.155, 0.160, 0.215, 1.0);
const ASFALTO_FAIXA: Color3D = Color3D::new(0.90, 0.94, 1.0, 1.0);
const GUIA_VERMELHA: Color3D = Color3D::new(0.95, 0.16, 0.30, 1.0);
const GUIA_BRANCA: Color3D = Color3D::new(0.70, 0.74, 0.82, 1.0);
const MURO: Color3D = Color3D::new(0.22, 0.23, 0.32, 1.0);
const NEON_CIANO: Color3D = Color3D::new(0.16, 0.92, 1.0, 1.0);
const NEON_MAGENTA: Color3D = Color3D::new(1.0, 0.22, 0.82, 1.0);
const PREDIO_A: Color3D = Color3D::new(0.20, 0.21, 0.30, 1.0);
const PREDIO_B: Color3D = Color3D::new(0.24, 0.20, 0.34, 1.0);
const PREDIO_C: Color3D = Color3D::new(0.17, 0.23, 0.32, 1.0);
/// Silhueta distante: mais escura e mais azul que os prédios da beira da pista,
/// para o horizonte ter profundidade em vez de terminar num bloco só.
const PREDIO_LONGE: Color3D = Color3D::new(0.10, 0.12, 0.22, 1.0);
const JANELA: Color3D = Color3D::new(1.0, 0.84, 0.48, 1.0);
const POSTE: Color3D = Color3D::new(0.28, 0.30, 0.40, 1.0);
const LUZ_POSTE: Color3D = Color3D::new(1.0, 0.94, 0.72, 1.0);
const TUNEL: Color3D = Color3D::new(0.16, 0.17, 0.23, 1.0);
const TUNEL_LUZ: Color3D = Color3D::new(0.85, 0.94, 1.0, 1.0);
const CONE: Color3D = Color3D::new(1.0, 0.46, 0.10, 1.0);
const LARGADA_A: Color3D = Color3D::new(0.95, 0.95, 1.0, 1.0);
const LARGADA_B: Color3D = Color3D::new(0.10, 0.10, 0.14, 1.0);
const VIDRO: Color3D = Color3D::new(0.07, 0.09, 0.16, 1.0);
const PNEU: Color3D = Color3D::new(0.045, 0.045, 0.06, 1.0);
const FAROL: Color3D = Color3D::new(0.92, 0.98, 1.0, 1.0);
const LANTERNA: Color3D = Color3D::new(1.0, 0.16, 0.22, 1.0);
const REFLEXO: Color3D = Color3D::new(0.35, 0.78, 1.0, 0.16);
/// Tints dos materiais **texturizados**.
///
/// A textura é cinza (ou verde, no caso da mata) e a cor final é `amostra × tint`,
/// então estes valores são o material, não a cor da textura. Um tint escuro num
/// material já escuro apaga a superfície — por isso o concreto é bem mais claro
/// aqui do que o `MURO` de cor chapada era.
const CONCRETO: Color3D = Color3D::new(0.44, 0.46, 0.56, 1.0);
const PEDRA: Color3D = Color3D::new(0.52, 0.50, 0.48, 1.0);
const MATA: Color3D = Color3D::new(0.30, 0.52, 0.30, 1.0);
const GUARDA_CORPO: Color3D = Color3D::new(0.62, 0.64, 0.70, 1.0);
const PAINEL_NEON: Color3D = Color3D::new(1.0, 0.55, 0.92, 1.0);
/// Luz de tudo o que é fonte de luz na cena: ambiente cheio e difusa zero.
///
/// É o que faz o neon parecer neon. Com a luz do mundo (ambiente 0,62) o ciano
/// saía a 45% e a avenida ficava cinza.
const EMISSIVO: DirectionalLight = DirectionalLight {
    direction: Vec3::new(0.0, -1.0, 0.0),
    color: Color3D::BLACK,
    ambient: 1.0,
    specular: 0.0,
    shininess: 1.0,
    _reserved: [0; 4],
};

/// Poça de luz do poste no asfalto molhado. Translúcida: com blend ligado no
/// quadro inteiro, ela soma luz sem apagar a faixa branca que passa por baixo.
const POCA_LUZ: Color3D = Color3D::new(0.42, 0.36, 0.20, 0.42);

/// Altura do plano do HUD, em unidades de mundo.
///
/// Com a câmera do HUD na origem olhando para -Z e `fov` de 90°, um ponto a esta
/// distância tem `x = ndc_x · PROF · aspecto` e `y = ndc_y · PROF`. É o que
/// permite posicionar HUD em pixels.
const PROFUNDIDADE_HUD: f32 = 10.0;

/// Campo de visão horizontal base, em radianos (~74°).
const FOV_BASE: f32 = 1.30;
/// Campo de visão com nitro no máximo (~92°): é o que dá a sensação de velocidade.
const FOV_NITRO: f32 = 1.62;

// ════════════════════════════════════════════════════════════
// CENA
// ════════════════════════════════════════════════════════════

/// Estado da câmera, com atraso e inclinação.
///
/// ## Por que a câmera não segue o carro na hora
///
/// Colada no carro, ela transmite cada correção de volante como um tranco e o
/// jogador perde a referência da pista. O atraso exponencial é o que dá o "peso"
/// de perseguição. A inclinação (`roll`) é o que faz a curva parecer curva: é o
/// mesmo truque do cinema, e custa uma multiplicação.
struct CameraJogo {
    pos: Vec3,
    alvo: Vec3,
    fov: f32,
    roll: f32,
    tremor: f32,
}

impl CameraJogo {
    fn nova(pista: &Pista, rotacao: Rotacao) -> Self {
        let inicio = pista.ponto_em(0.0);
        let (x, z) = girar_ponto(rotacao, inicio);
        let y = pista.altura_em(0.0);
        Self {
            pos: Vec3::new(x, y + 3.4, z - 8.0),
            alvo: Vec3::new(x, y + 1.0, z + 4.0),
            fov: FOV_BASE,
            roll: 0.0,
            tremor: 0.0,
        }
    }
}

/// As texturas do mapa, já na GPU.
///
/// Um `TextureHandle` por material. A cor continua vindo do `tint` do desenho —
/// a textura dá **relevo**, não cor — e é por isso que a mesma textura de
/// concreto serve ao muro azulado da avenida e à pedra clara da serra.
struct TexturasGpu {
    asfalto: giochi_engine_3d::TextureHandle,
    concreto: giochi_engine_3d::TextureHandle,
    pedra: giochi_engine_3d::TextureHandle,
    mata: giochi_engine_3d::TextureHandle,
    guarda: giochi_engine_3d::TextureHandle,
    painel: giochi_engine_3d::TextureHandle,
    xadrez: giochi_engine_3d::TextureHandle,
}

impl TexturasGpu {
    fn criar(render: &mut RenderContext3D) -> Result<Self, Box<dyn Error>> {
        let mut handles = Vec::new();
        let mut total = 0usize;
        let mut nomes = Vec::new();
        for t in crate::texturas::todas() {
            let h = render.create_texture_rgba(t.largura, t.altura, &t.pixels)?;
            total += t.bytes();
            nomes.push(t.nome);
            handles.push(h);
        }
        // Uma textura que falha em subir não tem sintoma visível: o material
        // desenha preto e o quadro continua "funcionando". Dizer o que subiu, o
        // tamanho e quantas são é o que transforma "está preto" em diagnóstico.
        println!(
            "neon-drift: {} textura(s) {nomes:?} {total} bytes, {} x {}",
            handles.len(),
            crate::texturas::LADO,
            crate::texturas::LADO
        );
        Ok(Self {
            asfalto: handles[0],
            concreto: handles[1],
            pedra: handles[2],
            mata: handles[3],
            guarda: handles[4],
            painel: handles[5],
            xadrez: handles[6],
        })
    }
}

/// A cena inteira: malhas, shaders e o estado de câmera/partículas.
pub struct Cena3D {
    // ── estáticos ──────────────────────────────────────────────
    /// Texturas do mapa, criadas no boot e vivas enquanto a cena viver.
    texturas: TexturasGpu,
    /// Shader que amostra textura em vez de usar cor por malha.
    texturizado: ShaderHandle,
    /// Malhas **texturizadas**: uma por material, uma chamada cada.
    asfalto_t: MeshHandle,
    concreto_t: MeshHandle,
    pedra_t: MeshHandle,
    mata_t: MeshHandle,
    guarda_t: MeshHandle,
    xadrez_t: MeshHandle,
    painel_t: MeshHandle,
    /// Cubo pequeno para os fragmentos de colisão, instanciado.
    cubo: MeshHandle,
    asfalto: MeshHandle,
    faixas: MeshHandle,
    guia_a: MeshHandle,
    guia_b: MeshHandle,
    muros: MeshHandle,
    neon: MeshHandle,
    neon_b: MeshHandle,
    reflexos: MeshHandle,
    poco_luz: MeshHandle,
    telhados: MeshHandle,
    longe: MeshHandle,
    predios_a: MeshHandle,
    predios_b: MeshHandle,
    predios_c: MeshHandle,
    janelas: MeshHandle,
    postes: MeshHandle,
    luzes: MeshHandle,
    tunel: MeshHandle,
    tunel_luzes: MeshHandle,
    tunel_luzes_b: MeshHandle,
    cones: MeshHandle,
    largada_b: MeshHandle,

    // ── carros ─────────────────────────────────────────────────
    corpo: MeshHandle,
    cabine: MeshHandle,
    rodas: MeshHandle,
    farois: MeshHandle,
    lanternas: MeshHandle,

    // ── dinâmicos ──────────────────────────────────────────────
    quad: MeshHandle,
    phong: ShaderHandle,
    instanciado: Option<ShaderHandle>,
    instancing: bool,

    rotacao: Rotacao,
    /// Medidas da tela de projeto no quadro corrente (paisagem nos dois sentidos).
    largura_projeto: f32,
    altura_projeto: f32,
    camera: CameraJogo,
    particulas: Particulas,
    /// Malhas de instância para os efeitos e para o HUD.
    malhas_efeito: Vec<Mat4>,
    instancias: Vec<Mat4>,
    /// Tempo acumulado, para animações (piscar do neon, tremor da câmera).
    tempo: f32,
    /// Luz do mundo, guardada para restaurar depois do HUD.
    luz_mundo: DirectionalLight,
    /// Cor do céu, que depende do cenário.
    ceu: Color3D,
    /// Quando `Some`, só esta malha é desenhada (diagnóstico `--so`).
    so: Option<String>,
    /// Impacto lido no quadro anterior.
    ///
    /// Serve para a rajada de destroços sair **uma vez por batida**: o impacto
    /// decai no núcleo, mas enquanto ele está acima do limiar o desenho o veria
    /// em vários quadros seguidos e soltaria destroços continuamente.
    ultimo_impacto: f32,
    /// Chamadas de desenho emitidas no quadro corrente.
    ///
    /// Existe porque **este** é o número que decide o quadro no RK3566: medido
    /// no aparelho, cada chamada custa ~0,15 ms de CPU e o orçamento de 60 Hz
    /// são 16,7 ms. Sem contar, "otimizar" vira palpite.
    desenhos: u32,
}

impl Cena3D {
    /// Restringe o desenho a uma malha, pelo nome do diagnóstico.
    pub fn so(&mut self, nome: Option<String>) {
        self.so = nome;
    }

    /// Monta a cena a partir da pista e do cenário do mapa.
    pub fn nova(
        render: &mut RenderContext3D,
        pista: &Pista,
        rotacao: Rotacao,
        cenario: Cenario,
    ) -> Result<Self, Box<dyn Error>> {
        let mundo = construir_mundo(pista, rotacao, cenario);

        let ceu = match cenario {
            Cenario::Avenida => NOITE,
            Cenario::Descida => CEU_SERRA,
        };
        let texturas = TexturasGpu::criar(render)?;
        let texturizado = render.create_builtin_shader(BuiltinShader::Textured)?;
        let asfalto_t = criar(render, &mundo.asfalto_t)?;
        let concreto_t = criar(render, &mundo.concreto_t)?;
        let pedra_t = criar(render, &mundo.pedra_t)?;
        let mata_t = criar(render, &mundo.mata_t)?;
        let guarda_t = criar(render, &mundo.guarda_t)?;
        let xadrez_t = criar(render, &mundo.xadrez_t)?;
        let painel_t = criar(render, &mundo.painel_t)?;
        let cubo = render.create_cube_mesh(1.0)?;

        let asfalto = criar(render, &mundo.asfalto)?;
        let faixas = criar(render, &mundo.faixas)?;
        let guia_a = criar(render, &mundo.guia_a)?;
        let guia_b = criar(render, &mundo.guia_b)?;
        let muros = criar(render, &mundo.muros)?;
        let neon = criar(render, &mundo.neon)?;
        let neon_b = criar(render, &mundo.neon_b)?;
        let reflexos = criar(render, &mundo.reflexos)?;
        let poco_luz = criar(render, &mundo.poco_luz)?;
        let telhados = criar(render, &mundo.telhados)?;
        let longe = criar(render, &mundo.longe)?;
        let predios_a = criar(render, &mundo.predios_a)?;
        let predios_b = criar(render, &mundo.predios_b)?;
        let predios_c = criar(render, &mundo.predios_c)?;
        let janelas = criar(render, &mundo.janelas)?;
        let postes = criar(render, &mundo.postes)?;
        let luzes = criar(render, &mundo.luzes)?;
        let tunel = criar(render, &mundo.tunel)?;
        let tunel_luzes = criar(render, &mundo.tunel_luzes)?;
        let tunel_luzes_b = criar(render, &mundo.tunel_luzes_b)?;
        let cones = criar(render, &mundo.cones)?;
        let largada_b = criar(render, &mundo.largada_b)?;

        let carro = malha::carro();
        let corpo = render.create_mesh(&carro.corpo.0, &carro.corpo.1)?;
        let cabine = render.create_mesh(&carro.cabine.0, &carro.cabine.1)?;
        let rodas = render.create_mesh(&carro.rodas.0, &carro.rodas.1)?;
        let farois = render.create_mesh(&carro.farois.0, &carro.farois.1)?;
        let lanternas = render.create_mesh(&carro.lanternas.0, &carro.lanternas.1)?;

        let (qv, qi) = malha::quad();
        let quad = render.create_mesh(&qv, &qi)?;

        let phong = render.create_builtin_shader(BuiltinShader::Phong)?;
        let instancing = render.supports_instancing();
        let instanciado = if instancing {
            Some(render.create_builtin_shader(BuiltinShader::Instanced)?)
        } else {
            None
        };

        // `sun()` puro (ambiente 0,3 + difusa 1,0 + especular 0,25) satura: o
        // que era branco virava 255,255,255 chapado e o que era claro virava
        // branco. Com 0,62 de ambiente e 0,34 de difusa a cor da malha continua
        // mandando, e ainda há variação entre faces — é o que dá volume ao
        // prédio e ao carro sem apagar a paleta.
        let luz_mundo = DirectionalLight::sun()
            .with_ambient(0.62)
            .with_color(Color3D::new(0.34, 0.34, 0.40, 1.0))
            .with_specular(0.10, 40.0);

        Ok(Self {
            texturas,
            texturizado,
            ceu,
            asfalto_t,
            concreto_t,
            pedra_t,
            mata_t,
            guarda_t,
            xadrez_t,
            painel_t,
            cubo,
            asfalto,
            faixas,
            guia_a,
            guia_b,
            muros,
            neon,
            neon_b,
            reflexos,
            poco_luz,
            telhados,
            longe,
            predios_a,
            predios_b,
            predios_c,
            janelas,
            postes,
            luzes,
            tunel,
            tunel_luzes,
            tunel_luzes_b,
            cones,
            largada_b,
            corpo,
            cabine,
            rodas,
            farois,
            lanternas,
            quad,
            phong,
            instanciado,
            instancing,
            rotacao,
            largura_projeto: 640.0,
            altura_projeto: 480.0,
            camera: CameraJogo::nova(pista, rotacao),
            particulas: Particulas::nova(),
            malhas_efeito: Vec::with_capacity(1024),
            instancias: Vec::with_capacity(512),
            tempo: 0.0,
            luz_mundo,
            so: None,
            ultimo_impacto: 0.0,
            desenhos: 0,
        })
    }

    /// Chamadas de desenho do último quadro.
    pub fn desenhos(&self) -> u32 {
        self.desenhos
    }

    /// `true` quando o driver aceita desenho instanciado.
    pub fn usa_instancing(&self) -> bool {
        self.instancing
    }

    /// Marca uma batida: a câmera treme por alguns décimos de segundo.
    ///
    /// Tremor e não flash: um flash branco num jogo noturno cega, e o tremor
    /// comunica "bateu" sem esconder a pista justamente na hora em que o jogador
    /// precisa corrigir.
    pub fn bater(&mut self) {
        self.camera.tremor = (self.camera.tremor + 0.9).min(1.4);
        // Faísca é o que **situa** a batida: o tremor sozinho diz "aconteceu
        // alguma coisa", mas não onde. A rajada sai das laterais do carro e
        // cai, então o olho encontra o ponto do choque.
        self.particulas.pedir_faiscas(14);
    }

    /// Cor de fundo (céu) e preparação do quadro.
    ///
    /// Precisa ser chamado **antes** de `begin_frame`: a engine aplica o estado
    /// dentro dele, e um `set_state` no meio do quadro só vale para o seguinte.
    /// Foi assim que o `blend` do neon e o `clear_color` da noite não pegavam.
    ///
    /// A mistura fica ligada para o quadro inteiro — os opacos têm alfa 1,0, e
    /// `src·1 + dst·0` é o próprio fragmento. Desligá-la exigiria dois estados
    /// por quadro, que esta engine não tem.
    ///
    /// O descarte de faces também fica desligado, pelo mesmo motivo: a corrida
    /// desenha caixas e quads virados para os dois lados (chão, billboards, HUD)
    /// e distinguir os passes custaria um estado por passe. O preço é desenhar a
    /// face de trás de caixas fechadas — que o teste de profundidade descarta —
    /// e o ganho é que um quadrilátero invertido nunca mais some em silêncio.
    pub fn preparar(&self, render: &mut RenderContext3D) {
        render.set_state(GlState {
            depth_test: true,
            cull_face: std::env::var_os("GIOCHI_CULL").is_none_or(|v| v != "0"),
            front_face: FrontFace::CounterClockwise,
            blend: true,
            clear_color: self.ceu,
        });
    }

    /// Desenha um quadro completo.
    pub fn desenhar(
        &mut self,
        render: &mut RenderContext3D,
        corrida: &Corrida,
        dt: f32,
    ) -> Result<(), Box<dyn Error>> {
        self.seguir(corrida, dt);
        self.desenhos = 0;

        // Duas luzes no mesmo quadro: `set_light` grava uniforme por desenho,
        // então dá para trocar entre um objeto e outro (o `set_state` não, que
        // só é aplicado no começo do quadro).
        //
        // Opacos são iluminados; tudo o que **emite** luz — neon, lampião,
        // janela, telhado, farol — é desenhado com luz cheia. Sem essa separação
        // o ciano do neon saía a 45% do brilho e a cidade ficava cinza.
        let camera = self.camera_mundo(render.drawable_size());
        let ident = Transform3D::IDENTITY;

        // ── mundo texturizado ────────────────────────────────────
        //
        // Uma chamada por material. O `tint` é o que dá a cor: a textura é
        // neutra de propósito, então o asfalto é escuro por tint e a pedra é
        // clara pela mesma textura com outro tint.
        //
        // Desenhado **primeiro**: o teste de profundidade resolve a ordem entre
        // opaco e opaco, e as superfícies texturizadas são todas opacas.
        render.set_light(self.luz_mundo);
        for (nome, malha, textura, tint) in [
            ("asfalto_t", self.asfalto_t, self.texturas.asfalto, ASFALTO),
            ("concreto_t", self.concreto_t, self.texturas.concreto, CONCRETO),
            ("pedra_t", self.pedra_t, self.texturas.pedra, PEDRA),
            ("mata_t", self.mata_t, self.texturas.mata, MATA),
            ("guarda_t", self.guarda_t, self.texturas.guarda, GUARDA_CORPO),
            ("painel_t", self.painel_t, self.texturas.painel, PAINEL_NEON),
            ("xadrez_t", self.xadrez_t, self.texturas.xadrez, LARGADA_A),
        ] {
            if self.quero(nome) && !self.desligado(parte_de_textura(nome)) {
                render.draw_mesh_textured(
                    malha,
                    self.texturizado,
                    &ident,
                    &camera,
                    textura,
                    tint,
                )?;
                self.desenhos += 1;
            }
        }

        render.set_light(self.luz_mundo);
        for (nome, malha, cor) in [
            ("asfalto", self.asfalto, ASFALTO),
            ("faixas", self.faixas, ASFALTO_FAIXA),
            ("guia_a", self.guia_a, GUIA_VERMELHA),
            ("guia_b", self.guia_b, GUIA_BRANCA),
            ("muros", self.muros, MURO),
            ("predios_a", self.predios_a, PREDIO_A),
            ("predios_b", self.predios_b, PREDIO_B),
            ("predios_c", self.predios_c, PREDIO_C),
            ("postes", self.postes, POSTE),
            ("tunel", self.tunel, TUNEL),
            ("cones", self.cones, CONE),
            ("largada_b", self.largada_b, LARGADA_B),
            ("longe", self.longe, PREDIO_LONGE),
        ] {
            if self.quero(nome) && !self.desligado(nome) {
                render.draw_mesh_colored(malha, self.phong, &ident, &camera, cor)?;
                self.desenhos += 1;
            }
        }

        // Pulso: a cidade respira. Custa **zero** — a cor já é uniforme por
        // desenho, então modular o brilho não é um desenho a mais nem um vertex a
        // mais. É o efeito mais barato que existe e o que mais diferencia uma
        // avenida estática de uma viva.
        let pulso = 0.80 + 0.20 * (self.tempo * 2.2).sin();
        let pulso_b = 0.80 + 0.20 * (self.tempo * 2.2 + std::f32::consts::PI).sin();
        let tremula = 0.94 + 0.06 * (self.tempo * 11.0).sin();

        render.set_light(EMISSIVO);
        for (nome, malha, cor) in [
            ("janelas", self.janelas, multiplicar(JANELA, tremula)),
            ("luzes", self.luzes, multiplicar(LUZ_POSTE, pulso)),
            ("telhados", self.telhados, multiplicar(NEON_MAGENTA, pulso_b)),
            ("tunel_luzes", self.tunel_luzes, multiplicar(TUNEL_LUZ, pulso)),
            ("tunel_luzes_b", self.tunel_luzes_b, multiplicar(TUNEL_LUZ, pulso_b)),
            ("neon", self.neon, multiplicar(NEON_CIANO, pulso)),
            ("neon_b", self.neon_b, multiplicar(NEON_MAGENTA, pulso_b)),
            ("poco_luz", self.poco_luz, multiplicar(POCA_LUZ, pulso)),
            ("reflexos", self.reflexos, multiplicar(REFLEXO, pulso)),
        ] {
            if self.quero(nome) && !self.desligado(nome) {
                render.draw_mesh_colored(malha, self.phong, &ident, &camera, cor)?;
                self.desenhos += 1;
            }
        }
        render.set_light(self.luz_mundo);

        // ── carros, partículas e HUD ──────────────────────────────
        if self.so.is_none() {
            if !self.desligado("carros") {
                self.desenhar_carros(render, corrida, &camera)?;
            }
            if !self.desligado("particulas") {
                self.desenhar_particulas(render, &camera)?;
                self.desenhar_fragmentos(render, &camera)?;
            }
            if !self.desligado("hud") {
                self.desenhar_hud(render, corrida)?;
            }
        }

        Ok(())
    }

    // ────────────────────────────────────────────────────────
    // câmera
    // ────────────────────────────────────────────────────────

    /// Avança só a câmera (e o tempo das animações), sem desenhar.
    ///
    /// Existe para o `--passos`: ele adianta a simulação e depois desenha **um**
    /// quadro, e sem isto a câmera ficava no ponto de partida enquanto o carro
    /// já estava 250 m adiante. O sintoma era um quadro que parecia defeito de
    /// desenho — a pista aparecia como uma faixa fina ao longe e a tela enchia
    /// de prédios.
    pub fn seguir(&mut self, corrida: &Corrida, dt: f32) {
        self.tempo += dt;
        self.atualizar_camera(corrida, dt);
        self.atualizar_particulas(corrida, dt);
    }

    /// Posição da câmera, para o diagnóstico em `--json`.
    pub fn camera(&self) -> (f32, f32, f32, f32) {
        (
            self.camera.pos.x,
            self.camera.pos.y,
            self.camera.pos.z,
            self.camera.fov,
        )
    }

    /// `GIOCHI_SEM=carros,particulas,hud` desliga partes do quadro.
    ///
    /// Existe para medir: num painel de 720×1280 o quadro é dominado por pixel,
    /// e sem separar as três partes "está lento" não diz o que cortar.
    fn desligado(&self, parte: &str) -> bool {
        std::env::var("GIOCHI_SEM")
            .map(|v| v.split(',').any(|p| p.trim() == parte))
            .unwrap_or(false)
    }

    fn quero(&self, nome: &str) -> bool {
        match &self.so {
            None => true,
            // `--so mundo` desenha toda a geometria estática e **nada** mais:
            // é o que separa "o mundo custa" de "carro, partícula e HUD custam".
            Some(filtro) if filtro == "mundo" => true,
            Some(filtro) => filtro == nome,
        }
    }

    fn atualizar_camera(&mut self, corrida: &Corrida, dt: f32) {
        let carro = &corrida.carro;
        let (px, pz) = girar_ponto(self.rotacao, carro.pos);
        let direcao = carro.direcao();
        let (dx, dz) = girar_direcao(self.rotacao, direcao);

        // Alvo: atrás do carro, e um pouco acima. A distância cresce um pouco com
        // a velocidade, o que abre o enquadramento na reta.
        //
        // O `y` soma a **altura da pista**: numa descida a câmera tem de descer
        // junto, senão ela fica no nível do mar enquanto o carro está a 60 m e o
        // jogo vira uma vista aérea.
        let rapidez = (carro.velocidade / 62.0).clamp(0.0, 1.4);
        let distancia = 7.4 + rapidez * 1.9;
        let altura = carro.altura + 3.05 + rapidez * 0.35;
        let alvo_pos = Vec3::new(px - dx * distancia, altura, pz - dz * distancia);
        let alvo_mira = Vec3::new(px + dx * 7.0, carro.altura + 1.05, pz + dz * 7.0);

        // Atraso exponencial estável em qualquer `dt`: `1 - exp(-k·dt)` não
        // estoura com quadro perdido, ao contrário de `k·dt`.
        let suavidade = 1.0 - (-8.5 * dt).exp();
        self.camera.pos = misturar(self.camera.pos, alvo_pos, suavidade);
        let suavidade_mira = 1.0 - (-11.0 * dt).exp();
        self.camera.alvo = misturar(self.camera.alvo, alvo_mira, suavidade_mira);

        // FOV: nitro abre o campo de visão. É o efeito de velocidade mais barato
        // que existe — nenhum pixel a mais é desenhado.
        let nitro = if carro.nitro_usando { 1.0 } else { 0.0 };
        let alvo_fov = FOV_BASE + (FOV_NITRO - FOV_BASE) * nitro;
        self.camera.fov += (alvo_fov - self.camera.fov) * (1.0 - (-6.0 * dt).exp());

        // Inclinação no drift: o horizonte tomba para o lado de fora da curva.
        let alvo_roll = (-carro.derrapagem * 0.010).clamp(-0.10, 0.10)
            + (carro.velocidade * 0.0015).clamp(0.0, 0.05);
        self.camera.roll += (alvo_roll - self.camera.roll) * (1.0 - (-7.0 * dt).exp());

        // Tremor de batida: some em ~0,25 s.
        self.camera.tremor = (self.camera.tremor - dt * 4.0).max(0.0);
    }

    /// Câmera do mundo, com a rotação do sistema aplicada como *roll*.
    fn camera_mundo(&self, tela: (u32, u32)) -> Camera3D {
        let aspecto = tela.0 as f32 / tela.1.max(1) as f32;
        // Com giro, a altura do painel cobre a largura do projeto: o FOV vertical
        // passa a ser o horizontal do projeto. Sem isso, o mundo sairia com o
        // zoom errado no X55 (retrato).
        let fov_y = match self.rotacao {
            Rotacao::Nenhuma => {
                2.0 * (self.camera.fov * 0.5).tan().atan2(aspecto)
            }
            _ => self.camera.fov,
        };

        let tremor = self.camera.tremor;
        let desvio = if tremor > 0.001 {
            Vec3::new(
                (self.tempo * 61.0).sin() * tremor * 0.35,
                (self.tempo * 47.0).cos() * tremor * 0.28,
                0.0,
            )
        } else {
            Vec3::ZERO
        };

        // `roll` = girar a câmera em torno do próprio eixo de visão. É o que faz
        // um painel retrato mostrar a cena paisagem sem tocar em nenhum vértice.
        let roll = match self.rotacao {
            Rotacao::Nenhuma => self.camera.roll,
            Rotacao::Cw90 => std::f32::consts::FRAC_PI_2 + self.camera.roll,
            Rotacao::Ccw90 => -std::f32::consts::FRAC_PI_2 + self.camera.roll,
        };
        let frente = direcao_normalizada(subtrair(self.camera.alvo, self.camera.pos));
        let up = Quat::from_axis_angle(frente, roll).rotate_vec3(Vec3::UP);

        Camera3D::from_parts(somar(self.camera.pos, desvio), self.camera.alvo, up)
            .with_fov_y(fov_y)
            .with_clip_planes(0.25, 900.0)
            .with_drawable_size(tela.0, tela.1)
    }

    // ────────────────────────────────────────────────────────
    // carros
    // ────────────────────────────────────────────────────────

    /// Desenha os quatro carros em **oito** chamadas, e não em vinte.
    ///
    /// ## Por que isso importa num RK3566
    ///
    /// Medido no aparelho: uma `draw_instanced` a mais custa ~0,15 ms de CPU, e
    /// o quadro inteiro tem 16,7 ms para caber em 60 Hz. Com cinco malhas por
    /// carro e quatro carros, o desenho ingênuo gastava 3 ms só aqui — 18% do
    /// orçamento em vinte chamadas que desenham a **mesma** geometria.
    ///
    /// Como as malhas são as mesmas, `draw_instanced` serve a todos os carros de
    /// uma vez: vidro, roda, farol e lanterna têm cor única e saem numa chamada
    /// cada. Só a lataria precisa de uma chamada por cor, porque a cor é
    /// uniforme por desenho.
    fn desenhar_carros(
        &mut self,
        render: &mut RenderContext3D,
        corrida: &Corrida,
        camera: &Camera3D,
    ) -> Result<(), Box<dyn Error>> {
        let Some(inst) = self.instanciado else {
            return Ok(());
        };

        // O jogador primeiro: se um adversário passar por cima, o carro do
        // jogador continua desenhado por cima dele.
        let mut todos: Vec<Mat4> = Vec::with_capacity(corrida.adversarios.len() + 1);
        let mut corpo_jogador: Vec<Mat4> = Vec::with_capacity(1);
        corpo_jogador.push(self.matriz_do_carro(
            corrida.carro.pos,
            corrida.carro.angulo,
            corrida.carro.altura,
        ));
        todos.push(corpo_jogador[0]);

        // Lataria dos adversários, agrupada por cor.
        let mut por_cor: Vec<(usize, Vec<Mat4>)> = Vec::new();
        for adversario in &corrida.adversarios {
            // A altura do adversário vem da pista no `s` dele: um carro
            // flutuando 40 m acima do asfalto na serra seria o defeito mais
            // visível do mundo, e nada no desenho avisaria.
            let m = self.matriz_do_carro(
                adversario.posicao(&corrida.pista),
                adversario.angulo(&corrida.pista),
                corrida.pista.altura_em(adversario.s),
            );
            todos.push(m);
            let cor = adversario.cor % CORES_ADVERSARIOS.len();
            match por_cor.iter_mut().find(|(c, _)| *c == cor) {
                Some((_, lista)) => lista.push(m),
                None => por_cor.push((cor, vec![m])),
            }
        }

        // Lataria: uma chamada por cor (1 do jogador + até 3 dos adversários).
        let jogador = Color3D::new(COR_JOGADOR[0], COR_JOGADOR[1], COR_JOGADOR[2], 1.0);
        render.draw_instanced(self.corpo, inst, &corpo_jogador, camera, jogador)?;
        self.desenhos += 1;
        for (indice, lista) in &por_cor {
            let c = CORES_ADVERSARIOS[*indice];
            render.draw_instanced(
                self.corpo,
                inst,
                lista,
                camera,
                Color3D::new(c[0], c[1], c[2], 1.0),
            )?;
            self.desenhos += 1;
        }

        // Peças de cor única: uma chamada para os quatro carros.
        for (malha, cor) in [
            (self.cabine, VIDRO),
            (self.rodas, PNEU),
            // Faróis e lanternas ficam acesos de qualquer ângulo: é o que faz o
            // carro ser visto de longe, à noite.
            (self.farois, FAROL),
            (self.lanternas, LANTERNA),
        ] {
            render.draw_instanced(malha, inst, &todos, camera, cor)?;
            self.desenhos += 1;
        }

        // ── aura de neon sob os carros ───────────────────────
        // Duas camadas porque não há textura nem gradiente: um quad só, opaco,
        // virava uma poça de 5×8 m que cobria 40% do quadro (medido). A de
        // dentro é pequena e forte, a de fora é o halo.
        //
        // O tamanho é medido em **pixels de tela**, não em metros: um quad
        // deitado de 5×10 m visto a 3 m de altura cobre um terço do quadro, e
        // como é translúcido ele paga cada pixel. Com 9,6 m de comprimento o
        // jogo caiu de 39 para 30 fps. 4,2 m cabe sob o carro, que é o que a
        // aura deve iluminar.
        self.instancias.clear();
        for m in todos.iter() {
            self.instancias.push(achatar(*m, 2.5, 4.2, 0.02));
        }
        render.draw_instanced(
            self.quad,
            inst,
            &self.instancias,
            camera,
            Color3D::new(jogador.r * 0.55, jogador.g * 0.55, jogador.b * 0.55, 0.42),
        )?;
        self.desenhos += 1;

        // ── feixe de farol no asfalto ────────────────────────
        // Um quad comprido à frente de cada carro, translúcido. É o que faz o
        // carro "abrir" a pista à noite; sem ele o farol ilumina a lataria e
        // não o chão.
        self.instancias.clear();
        for m in &todos {
            self.instancias.push(feixe(*m));
        }
        render.draw_instanced(
            self.quad,
            inst,
            &self.instancias,
            camera,
            Color3D::new(0.72, 0.84, 1.0, 0.16),
        )?;
        self.desenhos += 1;
        Ok(())
    }

    fn matriz_do_carro(&self, pos: Ponto, angulo: f32, altura: f32) -> Mat4 {
        let (x, z) = girar_ponto(self.rotacao, pos);
        let angulo = girar_angulo(self.rotacao, angulo);
        Transform3D::new(
            Vec3::new(x, altura, z),
            Quat::from_axis_angle(Vec3::UP, angulo),
            Vec3::new(1.0, 1.0, 1.0),
        )
        .matrix()
    }

    // ────────────────────────────────────────────────────────
    // partículas
    // ────────────────────────────────────────────────────────

    /// Avança o sistema de efeitos e entrega a câmera a ele.
    ///
    /// O sistema precisa da câmera porque chuva e poeira nascem **à frente
    /// dela**: emitir na volta inteira de 962 m daria 99% de partícula que
    /// ninguém vê e o dobro de trabalho.
    fn atualizar_particulas(&mut self, corrida: &Corrida, dt: f32) {
        self.particulas
            .atualizar(corrida, self.camera.pos, self.camera.alvo, dt);

        if corrida.fase != Fase::Correndo {
            return;
        }

        // ── raspão: faísca contínua ──────────────────────────
        //
        // O `raspando` do carro é **contato de parede**, não batida: encostar de
        // lado solta faísca sem tocar o som de colisão nem zerar o combo. Antes
        // disso o toque não deixava sinal nenhum, e o jogador não sabia se tinha
        // encostado ou se o carro tinha ficado mais lento por outro motivo.
        let carro = &corrida.carro;
        if carro.raspando && carro.velocidade.abs() > 6.0 {
            // Proporcional à velocidade e ao ângulo: raspão de lado solta pouco,
            // entrada torta solta muito.
            let forca = (carro.velocidade.abs() / 45.0).clamp(0.15, 1.0);
            let quantas = (forca * 3.0) as u32 + 1;
            self.particulas.pedir_faiscas(quantas);
        }

        // ── colisão: destroços ───────────────────────────────
        //
        // Só acima de 0,35: abaixo disso é raspão, e um encostar de 3 m/s não
        // pode jogar a mesma chuva de lataria que uma batida a 200 km/h. O
        // `impacto` já decai sozinho no núcleo, então a rajada sai **uma vez** por
        // batida em vez de todo quadro.
        const LIMIAR_DESTROCO: f32 = 0.35;
        if carro.impacto > LIMIAR_DESTROCO && carro.impacto >= self.ultimo_impacto {
            let (x, z) = girar_ponto(self.rotacao, carro.pos);
            let (dx, dz) = girar_direcao(self.rotacao, carro.direcao());
            self.particulas.pedir_fragmentos(
                carro.impacto,
                COR_JOGADOR,
                Vec3::new(x, carro.altura + 0.6, z),
                Vec3::new(dx, 0.0, dz),
            );
            self.camera.tremor = (self.camera.tremor + 0.5 * carro.impacto).min(1.4);
        }
        self.ultimo_impacto = if carro.impacto < 0.05 {
            0.0
        } else {
            carro.impacto
        };
    }

    /// Quantas partículas vivas (diagnóstico).
    pub fn particulas_vivas(&self) -> usize {
        self.particulas.total()
    }

    /// Destroços vivos (diagnóstico). Zero é resposta legítima: sem batida não
    /// há fragmento, e é isso que distingue "não desenhou" de "não bateu".
    pub fn fragmentos(&self) -> usize {
        self.particulas.fragmentos()
    }

    /// Contagem por tipo, na ordem de [`Tipo::TODOS`].
    pub fn contagem_particulas(&self) -> [u32; 7] {
        *self.particulas.contagens()
    }

    /// As contagens como objeto JSON, para o diagnóstico de `--json`.
    pub fn efeitos_json(&self) -> String {
        let contagem = self.contagem_particulas();
        Tipo::TODOS
            .iter()
            .zip(contagem.iter())
            .map(|(tipo, n)| format!("\"{}\":{n}", tipo.nome()))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// Esquece as partículas vivas (reinício de corrida).
    ///
    /// Sem isto a fumaça do drift anterior continua no ar depois do `ENTER`, e o
    /// jogador reinicia dentro da nuvem da corrida que ele acabou de abandonar.
    pub fn limpar_efeitos(&mut self) {
        self.particulas.limpar();
    }

    /// Desenha os destroços: **um** `draw_instanced` para todos.
    ///
    /// Cubos de verdade, com a matriz completa de cada peça (posição, giro em
    /// três eixos, escala). Um billboard sempre encara a câmera, e destroço que
    /// encara quem olha não lê como destroço.
    fn desenhar_fragmentos(
        &mut self,
        render: &mut RenderContext3D,
        camera: &Camera3D,
    ) -> Result<(), Box<dyn Error>> {
        let Some(inst) = self.instanciado else {
            return Ok(());
        };
        if self.particulas.fragmentos() == 0 {
            return Ok(());
        }
        self.particulas
            .matrizes_fragmentos(&mut self.malhas_efeito);
        if self.malhas_efeito.is_empty() {
            return Ok(());
        }
        let cor = self.particulas.cor_fragmentos();
        // Luz de verdade: destroço é objeto sólido, e com luz chapada o cubo
        // vira uma silhueta sem faces — some a leitura de "pedaço girando".
        render.set_light(self.luz_mundo);
        render.draw_instanced(self.cubo, inst, &self.malhas_efeito, camera, cor)?;
        self.desenhos += 1;
        Ok(())
    }

    /// Desenha as partículas: **uma** chamada de desenho por tipo vivo.
    ///
    /// Sete tipos no pior caso, e um tipo extinto não custa nada. A cor sai da
    /// vida restante de cada grupo, então a fumaça desbota sem precisar de um
    /// desenho por partícula.
    fn desenhar_particulas(
        &mut self,
        render: &mut RenderContext3D,
        camera: &Camera3D,
    ) -> Result<(), Box<dyn Error>> {
        if self.particulas.total() == 0 {
            return Ok(());
        }
        let Some(inst) = self.instanciado else {
            return Ok(());
        };

        // Billboards: os eixos vêm da câmera, então o quad sempre encara quem
        // olha. Um quad fixo sumiria de perfil.
        let (direita, cima) = efeitos::base_da_camera(camera.position, camera.target);

        // Luz chapada: partícula não recebe sombreamento, senão fumaça branca
        // vira fumaça cinza-escura no meio da pista mal iluminada.
        render.set_light(efeitos::LUZ_CHAPADA);

        for tipo in Tipo::TODOS {
            let total = efeitos::Particulas::faixas(tipo);
            let base = tipo.cor();
            for faixa in 0..total {
                let alfa = self.particulas.alfa_da_faixa(tipo, faixa, total);
                if alfa <= 0.004 {
                    continue;
                }
                self.particulas.matrizes(
                    tipo,
                    faixa,
                    total,
                    direita,
                    cima,
                    &mut self.malhas_efeito,
                );
                if self.malhas_efeito.is_empty() {
                    continue;
                }
                let cor = Color3D::new(base.r, base.g, base.b, alfa);
                render.draw_instanced(self.quad, inst, &self.malhas_efeito, camera, cor)?;
                self.desenhos += 1;
            }
        }

        render.set_light(self.luz_mundo);
        Ok(())
    }

    // ────────────────────────────────────────────────────────
    // HUD
    // ────────────────────────────────────────────────────────

    fn desenhar_hud(
        &mut self,
        render: &mut RenderContext3D,
        corrida: &Corrida,
    ) -> Result<(), Box<dyn Error>> {
        let Some(inst) = self.instanciado else {
            return Ok(());
        };
        let tela = render.drawable_size();
        // A rotação troca as medidas do HUD: o que se posiciona é a "tela de
        // projeto", que é paisagem nos dois casos.
        let projeto = match self.rotacao {
            Rotacao::Nenhuma => tela,
            _ => (tela.1, tela.0),
        };
        self.largura_projeto = projeto.0 as f32;
        self.altura_projeto = projeto.1 as f32;

        // O HUD não é iluminado: com `ambient = 1` e luz direta zero, a cor sai
        // exatamente como pedida. Sem isso o texto sairia 47 % mais escuro e
        // ninguém saberia por quê.
        render.set_light(DirectionalLight {
            direction: Vec3::new(0.0, 0.0, -1.0),
            color: Color3D::BLACK,
            ambient: 1.0,
            specular: 0.0,
            shininess: 1.0,
            _reserved: [0; 4],
        });
        let camera = camara_hud(projeto);
        let largura = projeto.0 as f32;
        let altura = projeto.1 as f32;

        match corrida.fase {
            Fase::Titulo => self.hud_titulo(render, inst, &camera, largura, altura)?,
            Fase::Correndo => self.hud_corrida(render, inst, &camera, corrida, largura, altura)?,
            Fase::Resultado => self.hud_resultado(render, inst, &camera, corrida, largura, altura)?,
        }

        // Linhas de velocidade por cima de tudo: é o "rastro" do nitro.
        self.hud_linhas_de_velocidade(render, inst, &camera, corrida, largura, altura)?;
        self.hud_raios_de_velocidade(render, inst, &camera, corrida, largura, altura)?;
        self.hud_flash_de_nitro(render, inst, &camera, corrida, largura, altura)?;

        render.set_light(self.luz_mundo);
        Ok(())
    }

    fn hud_titulo(
        &mut self,
        render: &mut RenderContext3D,
        inst: ShaderHandle,
        camera: &Camera3D,
        largura: f32,
        altura: f32,
    ) -> Result<(), Box<dyn Error>> {
        let escala = (largura / 200.0).clamp(2.0, 5.0);
        let titulo = fonte::largura_texto(NOME) as f32 * escala;
        self.texto(
            render,
            inst,
            camera,
            NOME,
            (largura - titulo) * 0.5,
            altura * 0.20,
            escala,
            NEON_CIANO,
        )?;

        let sub = fonte::largura_texto("CORRIDA NOTURNA ARCADE") as f32 * (escala * 0.45);
        self.texto(
            render,
            inst,
            camera,
            "CORRIDA NOTURNA ARCADE",
            (largura - sub) * 0.5,
            altura * 0.20 + escala * 10.0,
            escala * 0.45,
            NEON_MAGENTA,
        )?;

        let linhas = [
            "WASD OU SETAS  VIRAR",
            "ESPACO  FREIO DE MAO",
            "SHIFT  NITRO",
            "ENTER  JOGAR",
        ];
        let escala_corpo = (escala * 0.62).clamp(1.6, 3.4);
        let mut y = altura * 0.52;
        for linha in linhas {
            let w = fonte::largura_texto(linha) as f32 * escala_corpo;
            self.texto(
                render,
                inst,
                camera,
                linha,
                (largura - w) * 0.5,
                y,
                escala_corpo,
                Color3D::new(0.78, 0.84, 0.95, 1.0),
            )?;
            y += escala_corpo * 11.0;
        }

        // Pisca: é o que diferencia "instrução" de "espera por você".
        let pisca = 0.55 + 0.45 * (self.tempo * 4.0).sin();
        let dica = "ENTER PARA COMECAR";
        let w = fonte::largura_texto(dica) as f32 * escala_corpo;
        self.texto(
            render,
            inst,
            camera,
            dica,
            (largura - w) * 0.5,
            altura * 0.86,
            escala_corpo,
            Color3D::new(1.0, 0.95, 0.45, pisca),
        )?;
        Ok(())
    }

    fn hud_corrida(
        &mut self,
        render: &mut RenderContext3D,
        inst: ShaderHandle,
        camera: &Camera3D,
        corrida: &Corrida,
        largura: f32,
        altura: f32,
    ) -> Result<(), Box<dyn Error>> {
        let escala = (largura / 220.0).clamp(1.8, 4.2);

        // ── topo esquerdo: volta e posição ────────────────────────
        self.texto(
            render,
            inst,
            camera,
            &hud::volta(corrida.volta, corrida.total_voltas),
            largura * 0.03,
            altura * 0.04,
            escala,
            Color3D::new(0.85, 0.92, 1.0, 1.0),
        )?;
        self.texto(
            render,
            inst,
            camera,
            &hud::posicao(corrida.posicao, corrida.adversarios.len() + 1),
            largura * 0.03,
            altura * 0.04 + escala * 10.0,
            escala,
            if corrida.posicao == 1 { NEON_CIANO } else { NEON_MAGENTA },
        )?;

        // ── topo direito: tempo ──────────────────────────────────
        let tempo = hud::tempo(corrida.tempo);
        let w = fonte::largura_texto(&tempo) as f32 * escala;
        self.texto(
            render,
            inst,
            camera,
            &tempo,
            largura - w - largura * 0.03,
            altura * 0.04,
            escala,
            Color3D::new(0.85, 0.92, 1.0, 1.0),
        )?;

        // ── centro: drift ─────────────────────────────────────────
        if corrida.drift.ativo && corrida.drift.total() > 0 {
            let escala_drift = escala * 1.25;
            let texto = "DRIFT";
            let w = fonte::largura_texto(texto) as f32 * escala_drift;
            let pulso = 0.75 + 0.25 * (self.tempo * 12.0).sin();
            self.texto(
                render,
                inst,
                camera,
                texto,
                (largura - w) * 0.5,
                altura * 0.14,
                escala_drift,
                Color3D::new(NEON_MAGENTA.r, NEON_MAGENTA.g, NEON_MAGENTA.b, pulso),
            )?;
            let pontos = hud::pontos(corrida.drift.total());
            let wp = fonte::largura_texto(&pontos) as f32 * escala;
            self.texto(
                render,
                inst,
                camera,
                &pontos,
                (largura - wp) * 0.5,
                altura * 0.14 + escala_drift * 10.0,
                escala,
                Color3D::new(1.0, 0.86, 0.35, 1.0),
            )?;
            if corrida.drift.combo_inteiro() > 1 {
                let combo = hud::combo(corrida.drift.combo_inteiro());
                let wc = fonte::largura_texto(&combo) as f32 * escala;
                self.texto(
                    render,
                    inst,
                    camera,
                    &combo,
                    (largura - wc) * 0.5,
                    altura * 0.14 + escala_drift * 10.0 + escala * 9.0,
                    escala,
                    NEON_CIANO,
                )?;
            }
        }

        // ── rodapé esquerdo: velocímetro, marcha e nitro ──────────
        let base_y = altura - escala * 16.0;
        let velocidade = hud::velocidade(corrida.carro.kmh());
        let escala_vel = escala * 1.6;
        self.texto(
            render,
            inst,
            camera,
            &velocidade,
            largura * 0.03,
            base_y - escala_vel * 2.0,
            escala_vel,
            Color3D::new(1.0, 1.0, 1.0, 1.0),
        )?;
        let unidade = "KM/H";
        let wu = fonte::largura_texto(unidade) as f32 * (escala * 0.6);
        self.texto(
            render,
            inst,
            camera,
            unidade,
            largura * 0.03 + wu + fonte::largura_texto(&velocidade) as f32 * escala_vel + escala * 2.0,
            base_y - escala_vel,
            escala * 0.6,
            Color3D::new(0.7, 0.76, 0.9, 1.0),
        )?;

        // Marcha: um dígito grande à direita do velocímetro.
        let marcha = format!("{}", corrida.carro.marcha());
        self.texto(
            render,
            inst,
            camera,
            &marcha,
            largura * 0.03,
            base_y + escala * 2.0,
            escala * 1.2,
            NEON_CIANO,
        )?;

        // Barra de nitro: fundo escuro + preenchimento ciano + contorno.
        let barra_w = largura * 0.30;
        let barra_h = (escala * 3.0).max(6.0);
        let barra_x = largura * 0.03;
        let barra_y = base_y + escala * 6.5;
        self.retangulo(render, inst, camera, barra_x, barra_y, barra_w, barra_h, Color3D::new(0.10, 0.12, 0.20, 0.9))?;
        let cheio = barra_w * corrida.carro.nitro_fracao();
        if cheio > 1.0 {
            self.retangulo(render, inst, camera, barra_x, barra_y, cheio, barra_h, NEON_CIANO)?;
        }
        self.texto(
            render,
            inst,
            camera,
            "NITRO",
            barra_x + barra_w + escala * 2.0,
            barra_y - escala * 0.5,
            escala * 0.55,
            Color3D::new(0.7, 0.76, 0.9, 1.0),
        )?;

        // ── contagem regressiva ───────────────────────────────────
        if corrida.contagem > 0.0 {
            let numero = format!("{}", corrida.contagem.ceil().clamp(1.0, 3.0) as u32);
            let escala_conta = escala * 4.0;
            let w = fonte::largura_texto(&numero) as f32 * escala_conta;
            self.texto(
                render,
                inst,
                camera,
                &numero,
                (largura - w) * 0.5,
                altura * 0.36,
                escala_conta,
                Color3D::new(1.0, 0.9, 0.3, 1.0),
            )?;
        }

        Ok(())
    }

    fn hud_resultado(
        &mut self,
        render: &mut RenderContext3D,
        inst: ShaderHandle,
        camera: &Camera3D,
        corrida: &Corrida,
        largura: f32,
        altura: f32,
    ) -> Result<(), Box<dyn Error>> {
        let escala = (largura / 200.0).clamp(2.0, 5.0);

        let (titulo, cor) = match corrida.desfecho {
            Some(giochi_neon_drift_core::Desfecho::Vitoria) => ("VITORIA", NEON_CIANO),
            Some(giochi_neon_drift_core::Desfecho::ChegouDepois) => ("DERROTA", NEON_MAGENTA),
            Some(giochi_neon_drift_core::Desfecho::TempoEsgotado) => ("TEMPO ESGOTADO", NEON_MAGENTA),
            None => ("FIM", NEON_CIANO),
        };
        let w = fonte::largura_texto(titulo) as f32 * escala;
        self.texto(
            render,
            inst,
            camera,
            titulo,
            (largura - w) * 0.5,
            altura * 0.16,
            escala,
            cor,
        )?;

        let linhas = [
            format!("POSICAO {}", corrida.posicao),
            format!("TEMPO {}", hud::tempo(corrida.tempo)),
            format!("DRIFT {}", hud::pontos(corrida.drift.pontos)),
        ];
        let escala_corpo = escala * 0.7;
        let mut y = altura * 0.42;
        for linha in &linhas {
            let wl = fonte::largura_texto(linha) as f32 * escala_corpo;
            self.texto(
                render,
                inst,
                camera,
                linha,
                (largura - wl) * 0.5,
                y,
                escala_corpo,
                Color3D::new(0.88, 0.92, 1.0, 1.0),
            )?;
            y += escala_corpo * 12.0;
        }

        let pisca = 0.55 + 0.45 * (self.tempo * 4.0).sin();
        let dica = "ENTER  REINICIAR";
        let wd = fonte::largura_texto(dica) as f32 * escala_corpo;
        self.texto(
            render,
            inst,
            camera,
            dica,
            (largura - wd) * 0.5,
            altura * 0.82,
            escala_corpo,
            Color3D::new(1.0, 0.95, 0.45, pisca),
        )?;
        Ok(())
    }

    /// Linhas de velocidade nas laterais, mais longas com nitro.
    fn hud_linhas_de_velocidade(
        &mut self,
        render: &mut RenderContext3D,
        inst: ShaderHandle,
        camera: &Camera3D,
        corrida: &Corrida,
        largura: f32,
        altura: f32,
    ) -> Result<(), Box<dyn Error>> {
        if corrida.fase != Fase::Correndo {
            return Ok(());
        }
        let carro = &corrida.carro;
        let rapidez = (carro.velocidade / 62.0).clamp(0.0, 1.4);
        let nitro = carro.nitro_usando;
        let intensidade = (rapidez * 0.55 + if nitro { 0.75 } else { 0.0 }).clamp(0.0, 1.0);
        if intensidade < 0.12 {
            return Ok(());
        }

        self.instancias.clear();
        let quantas = (10.0 + intensidade * 22.0) as u32;
        for i in 0..quantas {
            let semente = (i as f32 * 12.9898 + self.tempo * 3.1).sin() * 43758.545;
            let aleatorio = semente - semente.floor();
            let lado = if i % 2 == 0 { 0.0 } else { 1.0 };
            let y = (aleatorio * altura * 0.9) + altura * 0.05;
            let comprimento = (largura * 0.05) * (0.5 + intensidade) * (0.4 + aleatorio);
            let x = if lado < 0.5 {
                largura * 0.02 + aleatorio * largura * 0.06
            } else {
                largura * 0.92 + aleatorio * largura * 0.06
            };
            self.instancias.push(hud_matriz(
                (largura, altura),
                x,
                y,
                comprimento,
                1.5 + intensidade * 1.5,
            ));
        }
        let cor = Color3D::new(0.75, 0.92, 1.0, 0.10 + intensidade * 0.22);
        render.draw_instanced(self.quad, inst, &self.instancias, camera, cor)?;
        Ok(())
    }


    /// Raios de velocidade saindo do ponto de fuga.
    ///
    /// É o "motion blur" barato: em vez de borrar a imagem (que exige
    /// render-to-texture e um passe de leitura do quadro anterior), o mesmo efeito
    /// de leitura vem de riscos longos que apontam para fora do centro, no ponto
    /// de fuga da pista. O olho lê direção e comprimento como movimento, e custa
    /// **uma** chamada de desenho.
    fn hud_raios_de_velocidade(
        &mut self,
        render: &mut RenderContext3D,
        inst: ShaderHandle,
        camera: &Camera3D,
        corrida: &Corrida,
        largura: f32,
        altura: f32,
    ) -> Result<(), Box<dyn Error>> {
        if corrida.fase != Fase::Correndo {
            return Ok(());
        }
        let carro = &corrida.carro;
        let rapidez = (carro.velocidade / 62.0).clamp(0.0, 1.4);
        let nitro = if carro.nitro_usando { 0.55 } else { 0.0 };
        let intensidade = (rapidez * 0.45 + nitro).clamp(0.0, 1.0);
        if intensidade < 0.18 {
            return Ok(());
        }

        let (cx, cy) = (largura * 0.5, altura * 0.46);
        // Fim do risco proporcional ao tamanho da tela, para o efeito ser o mesmo
        // em 640×480 e em 1280×720.
        let alcance = (largura * largura + altura * altura).sqrt() * 0.5;
        self.instancias.clear();
        let quantos = (14.0 + intensidade * 40.0) as u32;
        for i in 0..quantos {
            let semente = (i as f32 * 78.233 + self.tempo * 2.7).sin() * 43758.545;
            let a = semente - semente.floor();
            let angulo = a * std::f32::consts::TAU + i as f32 * 0.37;
            let inicio = alcance * (0.34 + 0.30 * a);
            let comprimento = alcance * (0.10 + 0.34 * intensidade) * (0.4 + a);
            let (s, c) = angulo.sin_cos();
            // O centro do quad fica no meio do segmento; o eixo longo aponta
            // radialmente para fora.
            let x = cx + c * (inicio + comprimento * 0.5);
            let y = cy + s * (inicio + comprimento * 0.5);
            self.instancias.push(hud_matriz_girada(
                (largura, altura),
                x,
                y,
                angulo,
                comprimento,
                1.6 + intensidade * 2.2,
            ));
        }
        let cor = Color3D::new(0.80, 0.94, 1.0, 0.06 + intensidade * 0.16);
        render.draw_instanced(self.quad, inst, &self.instancias, camera, cor)?;
        self.desenhos += 1;
        Ok(())
    }

    /// Clarão frio de tela cheia enquanto o nitro está aberto.
    ///
    /// Um quad do tamanho da tela com alfa baixo: o suficiente para o quadro
    /// "estourar" no acionamento. Barato (uma chamada) e é o que separa "usou o
    /// nitro" de "usou o nitro e o jogo respondeu".
    fn hud_flash_de_nitro(
        &mut self,
        render: &mut RenderContext3D,
        inst: ShaderHandle,
        camera: &Camera3D,
        corrida: &Corrida,
        largura: f32,
        altura: f32,
    ) -> Result<(), Box<dyn Error>> {
        if corrida.fase != Fase::Correndo || !corrida.carro.nitro_usando {
            return Ok(());
        }
        let pulso = 0.55 + 0.45 * (self.tempo * 18.0).sin();
        self.instancias.clear();
        self.instancias
            .push(hud_matriz((largura, altura), 0.0, 0.0, largura, altura));
        render.draw_instanced(
            self.quad,
            inst,
            &self.instancias,
            camera,
            Color3D::new(0.35, 0.75, 1.0, 0.05 + 0.05 * pulso),
        )?;
        self.desenhos += 1;
        Ok(())
    }

    /// Desenha um retângulo em pixels de projeto.
    fn retangulo(
        &mut self,
        render: &mut RenderContext3D,
        inst: ShaderHandle,
        camera: &Camera3D,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        cor: Color3D,
    ) -> Result<(), Box<dyn Error>> {
        self.instancias.clear();
        // A tela do HUD é o projeto: `hud_matriz` recebe as medidas em pixels.
        self.instancias.push(hud_matriz(
            (self.largura_projeto, self.altura_projeto),
            x,
            y,
            w,
            h,
        ));
        render.draw_instanced(self.quad, inst, &self.instancias, camera, cor)?;
        self.desenhos += 1;
        Ok(())
    }

    /// Desenha um texto de 5×7 pontos em pixels de projeto.
    #[allow(clippy::too_many_arguments)]
    fn texto(
        &mut self,
        render: &mut RenderContext3D,
        inst: ShaderHandle,
        camera: &Camera3D,
        texto: &str,
        x: f32,
        y: f32,
        escala: f32,
        cor: Color3D,
    ) -> Result<(), Box<dyn Error>> {
        let pontos = fonte::pontos(texto);
        if pontos.is_empty() {
            return Ok(());
        }
        self.instancias.clear();
        let tela = (self.largura_projeto, self.altura_projeto);
        // 1.06 de sobreposição: sem ela, cada ponto fica com uma fresta de fundo e
        // o texto parece serrilhado a partir de longe.
        let ponto = escala * 1.06;
        for [px, py] in pontos {
            self.instancias.push(hud_matriz(
                tela,
                x + px as f32 * escala,
                y + py as f32 * escala,
                ponto,
                ponto,
            ));
        }
        render.draw_instanced(self.quad, inst, &self.instancias, camera, cor)?;
        self.desenhos += 1;
        Ok(())
    }
}

// ════════════════════════════════════════════════════════════
// CONSTRUÇÃO DO MUNDO
// ════════════════════════════════════════════════════════════

/// Geometria estática separada por cor (uma malha por cor).
#[derive(Default)]
struct Mundo {
    /// Asfalto texturizado (sem cor por malha).
    asfalto_t: Construtor,
    /// Concreto texturizado: mureta da avenida, parede do túnel, prédios baixos.
    concreto_t: Construtor,
    /// Pedra texturizada: o paredão da serra.
    pedra_t: Construtor,
    /// Mata: as copas dos dois lados da estrada de serra.
    mata_t: Construtor,
    /// Guarda-corpo metálico da descida.
    guarda_t: Construtor,
    /// Xadrez da linha de largada.
    xadrez_t: Construtor,
    /// Outdoors e painéis de neon.
    painel_t: Construtor,
    asfalto: Construtor,
    faixas: Construtor,
    guia_a: Construtor,
    guia_b: Construtor,
    muros: Construtor,
    neon: Construtor,
    neon_b: Construtor,
    reflexos: Construtor,
    poco_luz: Construtor,
    telhados: Construtor,
    longe: Construtor,
    predios_a: Construtor,
    predios_b: Construtor,
    predios_c: Construtor,
    janelas: Construtor,
    postes: Construtor,
    luzes: Construtor,
    tunel: Construtor,
    tunel_luzes: Construtor,
    tunel_luzes_b: Construtor,
    cones: Construtor,
    largada_b: Construtor,
}

fn criar(render: &mut RenderContext3D, c: &Construtor) -> Result<MeshHandle, Box<dyn Error>> {
    // Malha vazia não vai para a GPU: `create_mesh` recusaria, e o erro
    // apareceria como falha de abertura do jogo.
    if c.vazio() {
        let (v, i) = malha::quad();
        return Ok(render.create_mesh(&v, &i)?);
    }
    Ok(render.create_mesh(c.vertices(), c.indices())?)
}

/// Gira um ponto do mundo para o espaço do projeto.
fn girar_ponto(rot: Rotacao, p: Ponto) -> (f32, f32) {
    match rot {
        Rotacao::Nenhuma => (p.x, p.z),
        // O giro da câmera já resolve a tela; o mundo fica como está.
        Rotacao::Cw90 | Rotacao::Ccw90 => (p.x, p.z),
    }
}

fn girar_direcao(rot: Rotacao, p: Ponto) -> (f32, f32) {
    girar_ponto(rot, p)
}

fn girar_angulo(rot: Rotacao, angulo: f32) -> f32 {
    match rot {
        Rotacao::Nenhuma | Rotacao::Cw90 | Rotacao::Ccw90 => angulo,
    }
}

/// Constrói a geometria estática do mapa, escolhendo o cenário pelo **tipo**
/// declarado nele.
///
/// O tipo não é o nome: dois mapas podem ser `Avenida` com traçados diferentes.
/// O que muda entre os dois cenários é o que existe em volta do asfalto — prédio
/// e outdoor de um lado, pedra e mata do outro — e é a única coisa do desenho
/// que precisa saber qual mapa está em jogo.
fn construir_mundo(pista: &Pista, rotacao: Rotacao, cenario: Cenario) -> Mundo {
    let mut m = Mundo::default();

    // A pista é comum aos dois: asfalto, faixas, guias, mureta e a tira de neon.
    construir_pista(&mut m, pista, rotacao);
    construir_largada(&mut m, pista, rotacao);
    construir_cones(&mut m, pista, rotacao);
    construir_tunel(&mut m, pista, rotacao);

    match cenario {
        Cenario::Avenida => {
            construir_arcos(&mut m, pista, rotacao);
            construir_passarelas(&mut m, pista, rotacao);
            construir_outdoors(&mut m, pista, rotacao);
            construir_cidade(&mut m, pista, rotacao);
            construir_longe(&mut m, pista, rotacao);
        }
        Cenario::Descida => {
            construir_serra(&mut m, pista, rotacao);
            construir_morros(&mut m, pista, rotacao);
            construir_cidade_ao_longe(&mut m, pista, rotacao);
        }
    }
    m
}

fn construir_pista(m: &mut Mundo, pista: &Pista, rotacao: Rotacao) {
    let amostras = pista.amostras();
    let tile = malha::Construtor::METROS_POR_TILE;
    for i in 0..amostras.len().saturating_sub(1) {
        let a = amostras[i];
        let b = amostras[i + 1];
        let (ax, az) = girar_ponto(rotacao, a.centro);
        let (bx, bz) = girar_ponto(rotacao, b.centro);
        let (anx, anz) = girar_ponto(rotacao, a.normal);
        let (bnx, bnz) = girar_ponto(rotacao, b.normal);
        // A altura vem da amostra: numa descida o vão entre duas amostras
        // atravessa o terreno, e usar um `y` só faria o asfalto degrauar.
        let (ya, yb) = (a.altura, b.altura);

        let al = (ax - anx * a.meia_largura, az - anz * a.meia_largura);
        let ar = (ax + anx * a.meia_largura, az + anz * a.meia_largura);
        let bl = (bx - bnx * b.meia_largura, bz - bnz * b.meia_largura);
        let br = (bx + bnx * b.meia_largura, bz + bnz * b.meia_largura);

        // Asfalto **texturizado**: uma malha, uma chamada, e o UV vem do mundo.
        m.asfalto_t
            .chao3([al.0, al.1], [bl.0, bl.1], [br.0, br.1], [ar.0, ar.1], ya, yb, yb, ya, tile);

        // Faixa central tracejada: 6 m acesa, 6 m apagada.
        let ciclo = (a.s / 12.0) as i32;
        if ciclo % 2 == 0 {
            let meia = 0.18;
            let al2 = (ax - anx * meia, az - anz * meia);
            let ar2 = (ax + anx * meia, az + anz * meia);
            let bl2 = (bx - bnx * meia, bz - bnz * meia);
            let br2 = (bx + bnx * meia, bz + bnz * meia);
            m.faixas.chao3(
                [al2.0, al2.1],
                [bl2.0, bl2.1],
                [br2.0, br2.1],
                [ar2.0, ar2.1],
                ya + 0.022,
                yb + 0.022,
                yb + 0.022,
                ya + 0.022,
                tile,
            );
        }

        // Guias alternadas vermelho/branco na borda, como zebra de curva.
        let largura_guia = 1.0;
        let deslocamento = a.meia_largura + largura_guia * 0.5;
        for lado in [-1.0f32, 1.0] {
            let a0 = (
                ax + anx * deslocamento * lado,
                az + anz * deslocamento * lado,
            );
            let b0 = (
                bx + bnx * deslocamento * lado,
                bz + bnz * deslocamento * lado,
            );
            let a1 = (
                ax + anx * (deslocamento + largura_guia) * lado,
                az + anz * (deslocamento + largura_guia) * lado,
            );
            let b1 = (
                bx + bnx * (deslocamento + largura_guia) * lado,
                bz + bnz * (deslocamento + largura_guia) * lado,
            );
            let malha = if lado > 0.0 {
                &mut m.guia_a
            } else {
                &mut m.guia_b
            };
            malha.chao3(
                [a0.0, a0.1],
                [b0.0, b0.1],
                [b1.0, b1.1],
                [a1.0, a1.1],
                ya + 0.012,
                yb + 0.012,
                yb + 0.012,
                ya + 0.012,
                tile,
            );
        }

        // Mureta: uma caixa a cada duas amostras, do lado de fora. Concreto
        // texturizado — é a superfície que o jogador vê de perto a corrida
        // inteira, e a que mais ganha com tijolo em vez de cor chapada.
        if i % 2 == 0 {
            let dist = a.meia_largura + 2.4;
            for lado in [-1.0f32, 1.0] {
                let (px, pz) = (ax + anx * dist * lado, az + anz * dist * lado);
                let (dx, dz) = girar_direcao(rotacao, a.direcao);
                let giro = dx.atan2(dz);
                let comprimento = (b.s - a.s).max(1.0) * 2.2;
                m.concreto_t.caixa_texturizada(
                    [px, ya + 0.45, pz],
                    [0.45, 0.9, comprimento],
                    giro,
                    tile,
                );
            }
        }

        // Refletor no topo da mureta, a cada quatro amostras. É o que dá a
        // sequência de pontos correndo na borda da tela — a leitura de
        // velocidade mais barata que existe numa avenida à noite.
        if i % 4 == 0 {
            let dist = a.meia_largura + 2.4;
            for lado in [-1.0f32, 1.0] {
                let (px, pz) = (ax + anx * dist * lado, az + anz * dist * lado);
                m.faixas
                    .caixa_girada([px, ya + 1.05, pz], [0.34, 0.22, 0.34], 0.0);
            }
        }

        // Tira de neon na borda: **contínua** e de duas cores, ciano de um lado
        // e magenta do outro. Era uma caixa a cada 3 amostras, 0,22 m de largura,
        // e cobria 0,8% do quadro — a cidade não lia como neon nenhum.
        {
            let (dx, dz) = girar_direcao(rotacao, a.direcao);
            let giro = dx.atan2(dz);
            let comprimento = ((b.s - a.s) * 1.15).max(0.6);
            for lado in [-1.0f32, 1.0] {
                let dist = a.meia_largura + 0.5;
                let (px, pz) = (ax + anx * dist * lado, az + anz * dist * lado);
                let malha = if lado > 0.0 {
                    &mut m.neon
                } else {
                    &mut m.neon_b
                };
                malha.caixa_girada([px, ya + 0.42, pz], [0.26, 0.72, comprimento], giro);

                // Reflexo: um quad deitado, mais largo, do lado de dentro. Sem
                // render-to-texture, o "asfalto molhado" é este brilho.
                m.reflexos.caixa_girada(
                    [px - anx * 1.9 * lado, ya + 0.012, pz - anz * 1.9 * lado],
                    [2.6, 0.012, comprimento * 0.95],
                    giro,
                );
            }
        }
    }
}

// ────────────────────────────────────────────────────────────
// CENÁRIO: serra (a descida)
// ────────────────────────────────────────────────────────────
//
// O que muda em relação à avenida não é a pista, é o que existe em volta:
//
// * **paredão de pedra** do lado de dentro da curva, em vez de prédio;
// * **mata** dos dois lados, em vez de janela iluminada;
// * **guarda-corpo** do lado de fora, porque numa descida o lado de fora é um
//   precipício e a mureta de concreto leria como avenida;
// * **a cidade lá embaixo**, longe e baixa, em vez de um skyline alto ao redor.
//
// Tudo isso entra nas malhas texturizadas que já existem — pedra, mata,
// guarda-corpo — então o cenário novo custa **zero** chamada de desenho.

fn construir_serra(m: &mut Mundo, pista: &Pista, rotacao: Rotacao) {
    let amostras = pista.amostras();
    let tile = malha::Construtor::METROS_POR_TILE;

    for i in 0..amostras.len().saturating_sub(1) {
        let a = amostras[i];
        let b = amostras[i + 1];
        let (ax, az) = girar_ponto(rotacao, a.centro);
        let (bx, bz) = girar_ponto(rotacao, b.centro);
        let (anx, anz) = girar_ponto(rotacao, a.normal);
        let (dx, dz) = girar_direcao(rotacao, a.direcao);
        let giro = dx.atan2(dz);
        let (ya, yb) = (a.altura, b.altura);

        // Dentro do túnel não há serra: a parede de pedra a 5 m do asfalto
        // atravessava o concreto e o quadro virava uma parede laranja sem
        // pista. O túnel tem geometria própria.
        if a.tunel || b.tunel {
            continue;
        }

        // ── guarda-corpo do lado de fora ─────────────────────
        // Chapa de 0,9 m com montante a cada duas amostras. Fica logo além da
        // guia, onde na avenida há mureta.
        if i % 2 == 0 {
            let dist = a.meia_largura + 0.9;
            for lado in [-1.0f32, 1.0] {
                let (px, pz) = (ax + anx * dist * lado, az + anz * dist * lado);
                m.guarda_t.caixa_texturizada(
                    [px, ya + 0.55, pz],
                    [0.12, 0.55, (b.s - a.s).max(1.0) * 2.1],
                    giro,
                    tile,
                );
            }
            // Montante: só do lado de fora, para o guarda-corpo não virar uma
            // cerca dos dois lados (a serra é de um lado só).
            let (px, pz) = (ax + anx * dist * -1.0, az + anz * dist * -1.0);
            m.guarda_t.caixa_texturizada(
                [px, ya + 0.30, pz],
                [0.16, 0.60, 0.16],
                0.0,
                tile,
            );
        }

        // ── paredão de pedra do lado de dentro ───────────────
        // Tira que acompanha o traçado, não caixa: a caixa de 19 m cortava a
        // curva por dentro (ver `parede3`). O lado vem da curvatura porque
        // numa serra a encosta fica sempre do lado de dentro da curva.
        let curvatura = curvatura_local(pista, a.s);
        let lado = if curvatura >= 0.0 { -1.0 } else { 1.0 };
        let dist = a.meia_largura + 6.0;
        let (pax, paz) = (ax + anx * dist * lado, az + anz * dist * lado);
        let (pbx, pbz) = (bx + anx * dist * lado, bz + anz * dist * lado);
        let altura = 5.0 + ((a.s * 0.03).sin() * 1.6).abs();
        let (nx, nz) = girar_ponto(rotacao, a.normal);
        m.pedra_t.parede3(
            [pax, paz],
            [pbx, pbz],
            ya - 0.4,
            yb - 0.4,
            altura,
            [nx * lado, nz * lado],
            tile * 2.0,
        );

        // ── mata dos dois lados ──────────────────────────────
        // Copas em duas fileiras, mais altas do lado de fora (a encosta desce).
        if i % 3 == 0 {
            let semente = (a.s * 0.31).sin() * 43758.545;
            let r = semente - semente.floor();
            for (fileira, lado_mata) in [(0usize, -1.0f32), (1, 1.0)] {
                let d = a.meia_largura + 11.0 + fileira as f32 * 8.0 + r * 5.0;
                let (px, pz) = (ax + anx * d * lado_mata, az + anz * d * lado_mata);
                let altura_copa = 5.0 + r * 5.0 + fileira as f32 * 2.5;
                m.mata_t.caixa_texturizada(
                    [px, ya - 2.5 + altura_copa * 0.5, pz],
                    [6.5, altura_copa, 6.5],
                    0.0,
                    tile * 1.6,
                );
            }
        }
    }
}

/// Curvatura local em `s`, positiva para a direita. Usada para decidir de que
/// lado fica o paredão: numa curva para a direita a serra está à esquerda.
fn curvatura_local(pista: &Pista, s: f32) -> f32 {
    let passo = 12.0;
    let a = pista.direcao_em(s);
    let b = pista.direcao_em(s + passo);
    let cruz = a.x * b.z - a.z * b.x;
    let escalar = a.ponto(b).clamp(-1.0, 1.0);
    cruz.signum() * escalar.acos()
}

/// Morros do outro lado do vale.
///
/// Sem eles o horizonte da serra é uma linha reta entre o asfalto e o céu, e a
/// descida perde a profundidade — o quadro fica com um terço de céu vazio em
/// cima e nada que diga "há um vale aqui". São caixas largas e baixas, muito
/// longe, com topo irregular; a 300 m, o que se vê é a silhueta.
fn construir_morros(m: &mut Mundo, pista: &Pista, rotacao: Rotacao) {
    let amostras = pista.amostras();
    let passo = 31;
    for (indice, a) in amostras.iter().enumerate().step_by(passo) {
        let semente = (indice as f32 * 3.117).sin() * 51247.19;
        let r = semente - semente.floor();
        let (px, pz) = girar_ponto(rotacao, a.centro);
        let (nx, nz) = girar_ponto(rotacao, a.normal);
        let (dx, dz) = girar_direcao(rotacao, a.direcao);
        let giro = dx.atan2(dz);
        for lado in [-1.0f32, 1.0] {
            let distancia = 210.0 + r * 170.0;
            let (bx, bz) = (px + nx * distancia * lado, pz + nz * distancia * lado);
            let na_pista = a.centro.mais(a.normal.vezes(distancia * lado));
            if pista.folga_em(na_pista) < 60.0 {
                continue;
            }
            // Base bem abaixo e topo variável: o que importa é a silhueta
            // contra o céu, e ela tem de ser irregular para não virar um muro.
            let altura = 55.0 + r * 70.0;
            let largura = 90.0 + r * 110.0;
            m.longe.caixa_girada(
                [bx, -50.0 + altura * 0.5, bz],
                [largura, altura, largura * 0.8],
                giro,
            );
        }
    }
}

/// A cidade **lá embaixo**: luzes baixas e distantes, sem prédio alto.
///
/// É o que dá a altura da serra. Um skyline alto ao redor diria "avenida"; pontos
/// de luz abaixo do nível da pista dizem "estou no morro".
fn construir_cidade_ao_longe(m: &mut Mundo, pista: &Pista, rotacao: Rotacao) {
    let amostras = pista.amostras();
    let passo = 17;
    for (indice, a) in amostras.iter().enumerate().step_by(passo) {
        let semente = (indice as f32 * 5.733).sin() * 31847.219;
        let r = semente - semente.floor();
        let (px, pz) = girar_ponto(rotacao, a.centro);
        let (nx, nz) = girar_ponto(rotacao, a.normal);
        let (dx, dz) = girar_direcao(rotacao, a.direcao);
        let giro = dx.atan2(dz);
        for lado in [-1.0f32, 1.0] {
            if r > 0.55 && lado > 0.0 {
                // Só um lado em metade das amostras: duas fileiras de torre
                // fechariam o horizonte e a serra perderia a vista.
                continue;
            }
            let distancia = a.meia_largura + 120.0 + r * 190.0;
            let (bx, bz) = (px + nx * distancia * lado, pz + nz * distancia * lado);
            let na_pista = a.centro.mais(a.normal.vezes(distancia * lado));
            if pista.folga_em(na_pista) < 30.0 {
                continue;
            }
            // **Abaixo** da pista: a cidade no fundo do vale. `-40` de base é o
            // que faz a diferença de altura existir na tela.
            let base = -42.0 + a.altura * 0.12;
            let altura = 12.0 + r * 26.0;
            let largura = 9.0 + r * 14.0;
            m.longe.caixa_girada(
                [bx, base + altura * 0.5, bz],
                [largura, altura, largura * 1.1],
                giro,
            );
            // Janelas: uma faixa fina e acesa em cada torre. A 150 m isso é o
            // suficiente para ler como cidade, e custa uma caixa.
            m.janelas.caixa_girada(
                [bx, base + altura * 0.72, bz],
                [largura * 1.02, 0.7, largura * 1.12],
                giro,
            );
        }
    }
}

// ────────────────────────────────────────────────────────────
// CENÁRIO: avenida (o circuito original)
// ────────────────────────────────────────────────────────────

fn construir_arcos(m: &mut Mundo, pista: &Pista, rotacao: Rotacao) {
    let amostras = pista.amostras();
    let mut proximo = 24.0f32;
    let mut indice_arco = 0usize;
    for i in 0..amostras.len().saturating_sub(1) {
        let a = amostras[i];
        if a.s < proximo {
            continue;
        }
        proximo += 26.0;
        if pista.em_tunel(a.s) {
            continue;
        }
        let (px, pz) = girar_ponto(rotacao, a.centro);
        let (nx, nz) = girar_ponto(rotacao, a.normal);
        let (dx, dz) = girar_direcao(rotacao, a.direcao);
        let giro = dx.atan2(dz);
        let vao = (a.meia_largura + 1.6) * 2.0;
        let y = a.altura;

        // Duas colunas e a viga. As colunas ficam **fora** da guia, para não
        // virarem obstáculo visual no meio da pista.
        for lado in [-1.0f32, 1.0] {
            let d = a.meia_largura + 1.6;
            let (cx, cz) = (px + nx * d * lado, pz + nz * d * lado);
            m.muros.caixa_girada([cx, y + 3.6, cz], [0.34, 7.2, 0.34], giro);
        }
        m.muros.caixa_girada([px, y + 7.15, pz], [vao, 0.42, 0.46], giro);

        // Tira de neon embaixo da viga, alternando ciano e magenta. Duas cores
        // alternadas custam zero: as duas malhas já existem.
        let malha = if indice_arco % 2 == 0 {
            &mut m.neon
        } else {
            &mut m.neon_b
        };
        malha.caixa_girada([px, y + 6.86, pz], [vao * 0.94, 0.13, 0.16], giro);
        // E o lampião da viga, que ilumina a pista de verdade.
        m.luzes
            .caixa_girada([px, y + 6.62, pz], [vao * 0.55, 0.10, 0.10], giro);
        indice_arco += 1;
    }
}

/// Três passarelas cruzando a pista. Referência de "trecho novo" e teto visual
/// que faz a cidade ter dois níveis em vez de um.
fn construir_passarelas(m: &mut Mundo, pista: &Pista, rotacao: Rotacao) {
    for s_alvo in [140.0f32, 470.0, 810.0] {
        let s = pista.normalizar_s(s_alvo);
        if pista.em_tunel(s) {
            continue;
        }
        let a = pista.ponto_lateral(s, 0.0);
        let normal = pista.normal_em(s);
        let direcao = pista.direcao_em(s);
        let (px, pz) = girar_ponto(rotacao, a);
        let (nx, nz) = girar_ponto(rotacao, normal);
        let (dx, dz) = girar_direcao(rotacao, direcao);
        let giro = dx.atan2(dz);
        let meia = pista.meia_largura_em(s);
        let vao = (meia + 5.0) * 2.0;
        let y = pista.altura_em(s);

        // Laje e as duas torres de apoio.
        m.concreto_t
            .caixa_texturizada([px, y + 8.6, pz], [vao, 0.9, 4.2], giro, 3.0);
        for lado in [-1.0f32, 1.0] {
            let d = meia + 5.0;
            let (cx, cz) = (px + nx * d * lado, pz + nz * d * lado);
            m.predios_c
                .caixa_girada([cx, y + 4.3, cz], [1.1, 8.6, 1.1], giro);
        }
        // Faixa de neon na face de baixo — é o que dá o "teto" luminoso.
        m.telhados
            .caixa_girada([px, y + 8.1, pz], [vao * 0.96, 0.16, 3.9], giro);
    }
}

/// Outdoors luminosos na beira da pista.
///
/// Placa alta com moldura de neon. A face é a **textura de painel** (faixas
/// diagonais com alfa), então o outdoor tem conteúdo em vez de ser um retângulo
/// chapado — e continua sem texto, que não há como desenhar numa caixa.
fn construir_outdoors(m: &mut Mundo, pista: &Pista, rotacao: Rotacao) {
    let amostras = pista.amostras();
    let mut proximo = 62.0f32;
    let mut n = 0usize;
    for a in amostras.iter() {
        if a.s < proximo {
            continue;
        }
        proximo += 62.0;
        if pista.em_tunel(a.s) || a.s < 40.0 {
            continue;
        }
        // Um lado por vez, alternando: dois outdoors na mesma altura dos dois
        // lados fechariam o quadro como um portal.
        let lado = if n % 2 == 0 { 1.0f32 } else { -1.0 };
        n += 1;
        let (px, pz) = girar_ponto(rotacao, a.centro);
        let (nx, nz) = girar_ponto(rotacao, a.normal);
        let (dx, dz) = girar_direcao(rotacao, a.direcao);
        let giro = dx.atan2(dz);
        let d = a.meia_largura + 8.0;
        let (bx, bz) = (px + nx * d * lado, pz + nz * d * lado);
        if pista.folga_em(a.centro.mais(a.normal.vezes(d * lado))) < 8.0 {
            continue;
        }
        let y = a.altura;

        // Estrutura, face texturizada e moldura.
        m.predios_c.caixa_girada([bx, y + 8.0, bz], [8.0, 5.2, 0.5], giro);
        m.painel_t.caixa_texturizada(
            [bx, y + 8.1, bz - 0.3 * lado],
            [7.0, 4.2, 0.12],
            giro,
            2.2,
        );
        m.neon_b.caixa_girada([bx, y + 10.7, bz], [8.2, 0.20, 0.14], giro);
        m.neon_b.caixa_girada([bx, y + 5.3, bz], [8.2, 0.20, 0.14], giro);
        // Mastro, para a placa não flutuar.
        m.postes.caixa_girada([bx, y + 4.0, bz], [0.26, 8.0, 0.26], giro);
    }
}

fn construir_cidade(m: &mut Mundo, pista: &Pista, rotacao: Rotacao) {
    let amostras = pista.amostras();
    let passo = 9;

    for (indice, a) in amostras.iter().enumerate().step_by(passo) {
        let (px, pz) = girar_ponto(rotacao, a.centro);
        let (nx, nz) = girar_ponto(rotacao, a.normal);
        let (dx, dz) = girar_direcao(rotacao, a.direcao);
        let giro = dx.atan2(dz);
        let y = a.altura;

        // Pseudoaleatório determinístico por índice: a cidade tem de ser a mesma
        // em todo quadro e em todo boot, senão a referência de comparação muda.
        let semente = (indice as f32 * 12.9898).sin() * 43758.5453;
        let r = semente - semente.floor();

        for lado in [-1.0f32, 1.0] {
            let distancia = a.meia_largura + 16.0 + r * 26.0;
            let altura = 10.0 + r * 34.0;
            let largura = 7.0 + r * 9.0;
            let (bx, bz) = (px + nx * distancia * lado, pz + nz * distancia * lado);

            // Prédio em cima de outro trecho da pista é o defeito que a cidade
            // aleatória produz sozinha num laço fechado: os do fim da volta
            // nasciam sobre a reta de largada e a câmera amanhecia dentro de um
            // deles. A folga é medida **no espaço da pista**.
            let na_pista = a.centro.mais(a.normal.vezes(distancia * lado));
            if pista.folga_em(na_pista) < largura * 0.75 + 4.0 {
                continue;
            }

            let grupo = ((indice / passo) + if lado > 0.0 { 1 } else { 0 }) % 3;
            let malha = match grupo {
                0 => &mut m.predios_a,
                1 => &mut m.predios_b,
                _ => &mut m.predios_c,
            };
            malha.caixa_girada(
                [bx, y + altura * 0.5, bz],
                [largura, altura, largura * (0.8 + r * 0.5)],
                giro,
            );

            // Faixa de neon no topo do prédio: de longe é o que desenha a
            // silhueta da cidade, e custa uma caixa por prédio.
            m.telhados.caixa_girada(
                [bx, y + altura + 0.35, bz],
                [largura * 0.92, 0.30, largura * 0.75],
                giro,
            );

            // Faixas de janelas: três caixas finas e brilhantes por prédio. É o
            // que faz a cidade parecer habitada à noite.
            let altura_janela = altura * 0.5;
            for k in 0..3 {
                let yj = y + altura * (0.22 + k as f32 * 0.26);
                let frente = (
                    bx + nx * (largura * 0.52) * -lado,
                    bz + nz * (largura * 0.52) * -lado,
                );
                m.janelas.caixa_girada(
                    [frente.0, yj, frente.1],
                    [largura * 0.62, 0.5 + altura_janela * 0.02, 0.35],
                    giro,
                );
            }

            // Poste de luz a cada dois prédios, mais perto da pista.
            if indice % (passo * 2) == 0 {
                let dp = a.meia_largura + 3.2;
                let (lx, lz) = (px + nx * dp * lado, pz + nz * dp * lado);
                m.postes.caixa_girada([lx, y + 3.4, lz], [0.22, 6.8, 0.22], 0.0);
                m.luzes.caixa_girada([lx, y + 6.9, lz], [0.9, 0.28, 0.9], 0.0);
                // Poça de luz no asfalto: é o que faz o poste existir à noite.
                // Sem ela, o lampião flutua e a rua continua preta embaixo dele.
                let (qx, qz) = (
                    px + nx * (a.meia_largura - 2.6) * lado,
                    pz + nz * (a.meia_largura - 2.6) * lado,
                );
                m.poco_luz
                    .caixa_girada([qx, y + 0.015, qz], [6.5, 0.015, 9.0], 0.0);
            }
        }
    }
}

/// Skyline distante: um anel de torres altas e escuras, longe da pista.
///
/// Sem ele o horizonte termina no último prédio e o céu fica um retângulo preto
/// do tamanho da tela. As torres são mais escuras e mais azuis que as da beira
/// — é o que dá a impressão de profundidade sem nenhum efeito de névoa.
fn construir_longe(m: &mut Mundo, pista: &Pista, rotacao: Rotacao) {
    let amostras = pista.amostras();
    let passo = 23;
    for (indice, a) in amostras.iter().enumerate().step_by(passo) {
        let semente = (indice as f32 * 7.919).sin() * 24634.6345;
        let r = semente - semente.floor();
        let (px, pz) = girar_ponto(rotacao, a.centro);
        let (nx, nz) = girar_ponto(rotacao, a.normal);
        let (dx, dz) = girar_direcao(rotacao, a.direcao);
        let giro = dx.atan2(dz);
        for lado in [-1.0f32, 1.0] {
            let distancia = a.meia_largura + 90.0 + r * 70.0;
            let (bx, bz) = (px + nx * distancia * lado, pz + nz * distancia * lado);
            let altura = 45.0 + r * 55.0;
            let largura = 18.0 + r * 16.0;
            let na_pista = a.centro.mais(a.normal.vezes(distancia * lado));
            if pista.folga_em(na_pista) < largura * 0.75 + 8.0 {
                continue;
            }
            m.longe
                .caixa_girada([bx, altura * 0.5, bz], [largura, altura, largura * 1.2], giro);
            // Faixa de neon na torre distante: dá andares ao bloco escuro, e a
            // 100 m da pista isso é toda a diferença entre parede e cidade.
            m.telhados.caixa_girada(
                [bx, altura * 0.62, bz],
                [largura * 0.72, 0.34, largura * 0.9],
                giro,
            );
        }
    }
}

fn construir_tunel(m: &mut Mundo, pista: &Pista, rotacao: Rotacao) {
    let amostras = pista.amostras();
    for i in 0..amostras.len().saturating_sub(1) {
        let a = amostras[i];
        if !a.tunel {
            continue;
        }
        let b = amostras[i + 1];
        let (ax, az) = girar_ponto(rotacao, a.centro);
        let (anx, anz) = girar_ponto(rotacao, a.normal);
        let (dx, dz) = girar_direcao(rotacao, a.direcao);
        let giro = dx.atan2(dz);
        let comprimento = (b.s - a.s).max(1.2) * 1.15;
        let y = a.altura;

        // Parede dos dois lados e teto, em concreto texturizado.
        for lado in [-1.0f32, 1.0] {
            let d = a.meia_largura + 1.4;
            let (px, pz) = (ax + anx * d * lado, az + anz * d * lado);
            m.concreto_t.caixa_texturizada(
                [px, y + 3.0, pz],
                [0.8, 6.0, comprimento],
                giro,
                3.0,
            );
        }
        let teto = 6.2;
        m.concreto_t.caixa_texturizada(
            [ax, y + teto, az],
            [a.meia_largura * 2.0 + 2.8, 0.5, comprimento],
            giro,
            3.0,
        );

        // Lâmpadas no teto, uma a cada duas amostras, alternando entre **duas**
        // malhas. Duas malhas de cor independente e pulsos em oposição de fase dão
        // a perseguição de luz sem nenhum estado por vértice — a engine só tem
        // cor uniforme por desenho, então a única forma de animar em sequência é
        // separar em grupos. Custa uma chamada.
        if i % 2 == 0 {
            let alvo = if i % 4 == 0 {
                &mut m.tunel_luzes
            } else {
                &mut m.tunel_luzes_b
            };
            alvo.caixa_girada(
                [ax, y + teto - 0.35, az],
                [a.meia_largura * 0.9, 0.16, 1.1],
                giro,
            );
        }
        // Faixa de luz nas paredes: o túnel precisa ser visível de dentro.
        if i % 3 == 0 {
            for lado in [-1.0f32, 1.0] {
                let d = a.meia_largura + 1.0;
                let (px, pz) = (ax + anx * d * lado, az + anz * d * lado);
                m.tunel_luzes.caixa_girada(
                    [px, y + 2.2, pz],
                    [0.12, 0.22, comprimento],
                    giro,
                );
            }
        }
    }
}

fn construir_cones(m: &mut Mundo, pista: &Pista, rotacao: Rotacao) {
    let tile = malha::Construtor::METROS_POR_TILE;
    for obstaculo in pista.obstaculos() {
        let p = pista.ponto_lateral(obstaculo.s, obstaculo.lateral);
        let (x, z) = girar_ponto(rotacao, p);
        let y = pista.altura_em(obstaculo.s);
        let _ = tile;
        m.cones.caixa(
            [x, y + obstaculo.raio * 0.6, z],
            [
                obstaculo.raio * 1.2,
                obstaculo.raio * 1.2,
                obstaculo.raio * 1.2,
            ],
        );
    }
}

fn construir_largada(m: &mut Mundo, pista: &Pista, rotacao: Rotacao) {
    // Quadriculado de 2 fileiras na linha de largada, em **textura de xadrez**:
    // é o único lugar do jogo onde a textura substitui 20 caixas.
    let meia = pista.meia_largura_em(0.0);
    let y = pista.altura_em(0.0);
    let (dx, dz) = girar_direcao(rotacao, pista.direcao_em(0.0));
    let giro = dx.atan2(dz);
    // A posição vem do **ponto da pista**, não da origem do mundo: a largada da
    // serra não fica em (0,0) e a primeira versão desenhava o xadrez no meio do
    // nada.
    let centro = pista.ponto_lateral(1.6, 0.0);
    let (cx, cz) = girar_ponto(rotacao, centro);
    m.xadrez_t
        .caixa_texturizada([cx, y + 0.02, cz], [meia * 2.0, 0.02, 3.2], giro, 1.6);
}

// ════════════════════════════════════════════════════════════
// MATRIZES DE HUD
// ════════════════════════════════════════════════════════════

/// Nome da parte que o diagnóstico `GIOCHI_SEM` usa para um material texturizado.
///
/// `--so` aceita o nome da malha ("asfalto_t"); `GIOCHI_SEM` usa o nome do
/// **material** ("asfalto"), porque é assim que se pensa ao medir: "sem asfalto".
fn parte_de_textura(nome: &str) -> &str {
    match nome {
        "asfalto_t" => "asfalto",
        "concreto_t" => "concreto",
        "pedra_t" => "pedra",
        "mata_t" => "mata",
        "guarda_t" => "guarda",
        "painel_t" => "painel",
        outro => outro,
    }
}

/// Multiplica o RGB por um fator, preservando o alfa.
fn multiplicar(cor: Color3D, k: f32) -> Color3D {
    Color3D::new(
        (cor.r * k).min(1.0),
        (cor.g * k).min(1.0),
        (cor.b * k).min(1.0),
        cor.a,
    )
}

/// Achata a matriz de um carro num quad deitado no chão.
///
/// O quad da engine é um plano em XY virado para +Z. A matriz de um carro gira
/// em torno de Y, então reusar as colunas dela mantém o quad **em pé** — foi o
/// que aconteceu com a primeira aura: um painel vertical de 5 m à frente do
/// carro, que cobria 40% do quadro e não lia como luz no chão. Aqui o eixo Y do
/// quad vira o eixo **frente** do carro (`-Z` local) e o eixo X vira a direita,
/// o que deita o quad no asfalto.
fn achatar(m: Mat4, largura: f32, comprimento: f32, y: f32) -> Mat4 {
    let d = m.data;
    // **Zero, não identidade.** A identidade deixa `m[1]` e `m[5]` em 1,0, e
    // esses dois campos são a componente Y dos eixos X e Y do quad: o resultado
    // é um cisalhamento de 1 m que inclina o quad para cima. Foi assim que a
    // aura do carro virou um painel vertical de 5 m em vez de uma poça no chão.
    let mut q = [0.0f32; 16];
    q[10] = 1.0;
    q[15] = 1.0;
    q[0] = d[0] * largura;
    q[2] = d[2] * largura;
    // `-d[8..10]` é a frente do carro: o modelo aponta para -Z no espaço local.
    q[4] = -d[8] * comprimento;
    q[6] = -d[10] * comprimento;
    q[12] = d[12];
    q[13] = y;
    q[14] = d[14];
    Mat4::from_cols_array(&q)
}

/// Retângulo do HUD **girado** no plano da tela, centrado em `(cx, cy)` pixels.
///
/// `hud_matriz` só faz retângulo alinhado aos eixos; os raios de velocidade
/// precisam apontar para fora do centro. A rotação é feita em espaço de pixel e
/// depois convertida, porque girar em espaço de NDC distorceria o ângulo quando
/// a tela não é quadrada.
fn hud_matriz_girada(
    tela: (f32, f32),
    cx: f32,
    cy: f32,
    angulo: f32,
    comprimento: f32,
    espessura: f32,
) -> Mat4 {
    let aspecto = tela.0 / tela.1.max(1.0);
    let ndc_x = cx / tela.0 * 2.0 - 1.0;
    let ndc_y = 1.0 - cy / tela.1 * 2.0;
    let (s, c) = angulo.sin_cos();
    // Eixos em pixel: o curto é perpendicular ao raio, o longo aponta para fora.
    let (curto_x, curto_y) = (-s * espessura, c * espessura);
    let (longo_x, longo_y) = (c * comprimento, s * comprimento);
    // Pixel → unidades da câmera do HUD (ver `hud_matriz`).
    let kx = PROFUNDIDADE_HUD * aspecto / tela.0;
    let ky = PROFUNDIDADE_HUD / tela.1;

    let mut m = Mat4::IDENTITY.to_cols_array();
    m[0] = curto_x * kx;
    m[1] = -curto_y * ky;
    m[4] = longo_x * kx;
    m[5] = -longo_y * ky;
    m[12] = ndc_x * PROFUNDIDADE_HUD * aspecto;
    m[13] = ndc_y * PROFUNDIDADE_HUD;
    m[14] = -PROFUNDIDADE_HUD;
    Mat4::from_cols_array(&m)
}

/// Quad deitado à frente do carro, no chão: o feixe do farol.
fn feixe(m: Mat4) -> Mat4 {
    let d = m.data;
    let mut q = [0.0f32; 16];
    q[10] = 1.0;
    q[15] = 1.0;
    q[0] = d[0] * 2.6;
    q[2] = d[2] * 2.6;
    q[4] = -d[8] * 7.5;
    q[6] = -d[10] * 7.5;
    // 5,5 m à frente do para-choque, para o feixe não nascer debaixo do carro.
    // Curto de propósito: 15 m de feixe translúcido no chão custam ~8 fps neste
    // painel, e o que se ganha em alcance se perde em quadro.
    q[12] = d[12] - d[8] * 5.5;
    q[13] = 0.03;
    q[14] = d[14] - d[10] * 5.5;
    Mat4::from_cols_array(&q)
}

/// Matriz de um retângulo de HUD, posicionado em **pixels** da tela de projeto.
fn hud_matriz(tela: (f32, f32), x: f32, y: f32, w: f32, h: f32) -> Mat4 {
    let aspecto = tela.0 / tela.1.max(1.0);
    let ndc_x = (x + w * 0.5) / tela.0 * 2.0 - 1.0;
    let ndc_y = 1.0 - (y + h * 0.5) / tela.1 * 2.0;
    Transform3D::new(
        Vec3::new(
            ndc_x * PROFUNDIDADE_HUD * aspecto,
            ndc_y * PROFUNDIDADE_HUD,
            -PROFUNDIDADE_HUD,
        ),
        Quat::IDENTITY,
        Vec3::new(
            (w / tela.0) * PROFUNDIDADE_HUD * aspecto,
            (h / tela.1) * PROFUNDIDADE_HUD,
            1.0,
        ),
    )
    .matrix()
}

/// Câmera ortográfica-de-fato do HUD: `fov` de 90° e plano a `PROFUNDIDADE_HUD`.
fn camara_hud(tela: (u32, u32)) -> Camera3D {
    Camera3D::from_parts(Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0), Vec3::UP)
        .with_fov_y(std::f32::consts::FRAC_PI_2)
        // Plano próximo em 9,0 com o HUD a 10: a profundidade normalizada do HUD
        // fica ~0,10, à frente de qualquer geometria do mundo (≥0,87 a partir de
        // 2 m). Era 0,5..60 e dava ~0,96 — o HUD ficava **atrás** de um prédio a
        // 30 m, e o sintoma era "o texto some quando a cidade chega perto".
        .with_clip_planes(9.0, 4000.0)
        .with_drawable_size(tela.0, tela.1)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::vetor::produto_vetorial;

    #[test]
    fn a_matriz_do_hud_mapeia_os_cantos_da_tela() {
        // Canto superior esquerdo de uma tela 640×480, um retângulo de 1 px: o
        // centro tem de cair no NDC (-1, 1) e o tamanho tem de ser 1 px.
        let tela = (640.0, 480.0);
        let m = hud_matriz(tela, 0.0, 0.0, 1.0, 1.0);
        let x = m.data[12];
        let y = m.data[13];
        let aspecto = 640.0 / 480.0;
        // ndc_x = -1 + 1/640, ndc_y = 1 - 1/480
        let ndc_x = x / (PROFUNDIDADE_HUD * aspecto);
        let ndc_y = y / PROFUNDIDADE_HUD;
        assert!((ndc_x - (-1.0 + 1.0 / 640.0)).abs() < 1e-4, "ndc_x {ndc_x}");
        assert!((ndc_y - (1.0 - 1.0 / 480.0)).abs() < 1e-4, "ndc_y {ndc_y}");
        assert!((m.data[14] + PROFUNDIDADE_HUD).abs() < 1e-5);
    }

    #[test]
    fn um_retangulo_do_tamanho_da_tela_cobre_o_ndc_inteiro() {
        let tela = (640.0, 480.0);
        let m = hud_matriz(tela, 0.0, 0.0, 640.0, 480.0);
        let aspecto = 640.0 / 480.0;
        // Largura em NDC = escala_x / (PROF * aspecto) * 2
        let largura_ndc = (m.data[0] / (PROFUNDIDADE_HUD * aspecto)) * 2.0;
        let altura_ndc = (m.data[5] / PROFUNDIDADE_HUD) * 2.0;
        assert!((largura_ndc - 2.0).abs() < 1e-4, "largura {largura_ndc}");
        assert!((altura_ndc - 2.0).abs() < 1e-4, "altura {altura_ndc}");
    }

    #[test]
    fn o_billboard_fica_centrado_na_particula() {
        let m =
            efeitos::matriz_billboard(Vec3::new(3.0, 2.0, -4.0), Vec3::new(1.0, 0.0, 0.0), Vec3::UP, 0.5);
        assert_eq!(m.data[12], 3.0);
        assert_eq!(m.data[13], 2.0);
        assert_eq!(m.data[14], -4.0);
        assert_eq!(m.data[0], 0.5);
        assert_eq!(m.data[5], 0.5);
    }

    #[test]
    #[test]
    fn o_quad_deitado_do_carro_fica_com_a_normal_para_cima() {
        // A aura e o feixe do farol são quads no chão. Normal para baixo + face
        // culling = efeito invisível, e o defeito não dá erro nenhum.
        for angulo in [0.0f32, 1.1, -2.3, 3.0] {
            let m = Transform3D::new(
                Vec3::ZERO,
                Quat::from_axis_angle(Vec3::UP, angulo),
                Vec3::new(1.0, 1.0, 1.0),
            )
            .matrix();
            for q in [achatar(m, 2.0, 4.0, 0.02), feixe(m)] {
                let a = Vec3::new(q.data[0], q.data[1], q.data[2]);
                let b = Vec3::new(q.data[4], q.data[5], q.data[6]);
                let n = produto_vetorial(a, b).normalized();
                assert!(n.y > 0.99, "ângulo {angulo}: normal {n:?}");
            }
        }
    }

    #[test]
    fn a_camera_do_hud_olha_para_menos_z() {
        let c = camara_hud((640, 480));
        assert_eq!(c.position, Vec3::ZERO);
        assert_eq!(c.target, Vec3::new(0.0, 0.0, -1.0));
        assert!(c.aspect > 1.3 && c.aspect < 1.4, "aspecto {}", c.aspect);
    }
}
