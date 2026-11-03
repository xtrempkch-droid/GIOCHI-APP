//! Volume do mixer, com persistência entre boots.
//!
//! ## Por que o app controla ALSA
//!
//! O mixer é interface **padrão** do Linux (`/dev/snd/controlC0` + libasound),
//! do mesmo jeito que o backlight é `/sys/class/backlight`. Não é remenda de
//! aparelho: qualquer sistema Linux com ALSA responde igual. O que seria
//! remenda é fixar o nome do elemento ("Master", "PCM", "Playback") sem olhar o
//! que o hardware oferece — então o elemento é **descoberto**, não presumido.
//!
//! ## Por que persistir aqui
//!
//! O sistema **não** guarda estado ALSA: não há `alsactl` no init nem
//! `/var/lib/alsa/asound.state`. Sem persistência, o volume volta ao padrão do
//! hardware a cada boot. O app grava `/user/.config/giochi/volume` e restaura na
//! abertura — mesmo contrato já usado para o brilho.
//!
//! ## Por que a lógica fica separada do FFI
//!
//! `Volume` genérico sobre [`Mixer`] permite testar passo, limite e persistência
//! sem placa de som. O `unsafe` fica confinado em [`AlsaMixer`] e é a única
//! parte que depende de hardware.

use std::cell::RefCell;

/// Onde o volume escolhido é gravado.
pub const VOLUME_SALVO: &str = "/user/.config/giochi/volume";

/// Interface de mixer. Implementada por ALSA e por um dublê de teste.
pub trait Mixer {
    /// Volume atual em 0..=100. `None` quando não há mixer.
    fn get(&self) -> Option<u8>;
    /// Aplica volume em 0..=100.
    fn set(&self, pct: u8);
}

impl<M: Mixer + ?Sized> Mixer for &M {
    fn get(&self) -> Option<u8> {
        (**self).get()
    }
    fn set(&self, pct: u8) {
        (**self).set(pct)
    }
}

/// Volume com passo, limite e memória do que foi aplicado.
pub struct Volume<M: Mixer> {
    mixer: M,
    /// Último valor aplicado, para o ajuste relativo não depender de reler o
    /// hardware a cada toque (ler é barato, mas dois toques rápidos podem
    /// intercalar e perder um passo).
    atual: u8,
    /// `false` quando o hardware não tem mixer: a UI continua funcionando,
    /// só não há som para ajustar.
    disponivel: bool,
}

impl<M: Mixer> Volume<M> {
    pub fn novo(mixer: M) -> Self {
        let (atual, disponivel) = match mixer.get() {
            Some(v) => (v, true),
            None => (50, false),
        };
        Self {
            mixer,
            atual,
            disponivel,
        }
    }

    pub fn disponivel(&self) -> bool {
        self.disponivel
    }

    pub fn atual(&self) -> u8 {
        self.atual
    }

    /// Define o volume absoluto (slider).
    pub fn definir(&mut self, pct: u8) -> u8 {
        let pct = pct.min(100);
        // Sem mudança real não escreve: evita I/O à toa quando o slider passa
        // pelo mesmo valor em arrasto.
        if pct == self.atual && self.disponivel {
            return self.atual;
        }
        self.atual = pct;
        if self.disponivel {
            self.mixer.set(pct);
            self.persistir(pct);
        }
        self.atual
    }

    /// Reaplica o volume salvo de uma sessão anterior.
    pub fn restaurar_salvo(&mut self) {
        let Some(pct) = ler_salvo() else {
            return;
        };
        self.atual = pct;
        if self.disponivel {
            self.mixer.set(pct);
        }
    }

    fn persistir(&self, pct: u8) {
        if let Some(dir) = std::path::Path::new(VOLUME_SALVO).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(VOLUME_SALVO, pct.to_string());
    }
}

fn ler_salvo() -> Option<u8> {
    let texto = std::fs::read_to_string(VOLUME_SALVO).ok()?;
    texto.trim().parse::<u8>().ok().filter(|v| *v <= 100)
}

// ── ALSA ────────────────────────────────────────────────────
//
// FFI manual em vez do crate `alsa`: o crate exige pkg-config e cabeçalhos de
// desenvolvimento no alvo, que o aparelho não tem. Estas dez funções são a
// superfície mínima do mixer e existem em qualquer libasound 2.x.

#[cfg(target_os = "linux")]
mod alsa {
    use super::Mixer;
    use std::ffi::CString;
    use std::ffi::{c_char, c_int, c_long, c_void};

    #[link(name = "asound")]
    extern "C" {
        fn snd_mixer_open(mixer: *mut *mut c_void, mode: c_int) -> c_int;
        fn snd_mixer_attach(mixer: *mut c_void, name: *const c_char) -> c_int;
        fn snd_mixer_selem_register(
            mixer: *mut c_void,
            options: *mut c_void,
            classp: *mut *mut c_void,
        ) -> c_int;
        fn snd_mixer_load(mixer: *mut c_void) -> c_int;
        fn snd_mixer_first_elem(mixer: *mut c_void) -> *mut c_void;
        fn snd_mixer_elem_next(elem: *mut c_void) -> *mut c_void;
        fn snd_mixer_selem_has_playback_volume(elem: *mut c_void) -> c_int;
        fn snd_mixer_selem_get_name(elem: *mut c_void) -> *const c_char;
        fn snd_mixer_selem_get_playback_volume(
            elem: *mut c_void,
            channel: c_int,
            value: *mut c_long,
        ) -> c_int;
        fn snd_mixer_selem_get_playback_volume_range(
            elem: *mut c_void,
            min: *mut c_long,
            max: *mut c_long,
        ) -> c_int;
        fn snd_mixer_selem_set_playback_volume_all(elem: *mut c_void, value: c_long) -> c_int;
        fn snd_mixer_close(mixer: *mut c_void) -> c_int;
    }

    /// `SND_MIXER_SCHN_FRONT_LEFT`.
    const CANAL_FRENTE_ESQ: c_int = 0;

    /// Elementos preferidos, na ordem em que costumam ser o volume principal.
    ///
    /// Ordem de preferência, não exigência: se nenhum existir, cai no primeiro
    /// elemento que tenha volume de reprodução. Fixar um nome seria remenda —
    /// o nome do controle é decisão do codec, não do app.
    const PREFERIDOS: [&str; 3] = ["Master", "PCM", "Playback"];

    /// Conexão viva com o mixer do `card0`.
    pub struct AlsaMixer {
        mixer: *mut c_void,
        /// Elemento escolhido. Copiado do ponteiro devolvido pela libasound,
        /// que só é válido enquanto `mixer` estiver aberto.
        elem: *mut c_void,
    }

    // `AlsaMixer` é usado de uma thread só (o laço da UI). A libasound não é
    // thread-safe por handle, então mover entre threads seria enganoso — por
    // isso o app o mantém local ao laço, sem `Arc` nem `Send`.

    impl AlsaMixer {
        pub fn abrir(card: &str) -> Option<Self> {
            let nome = CString::new(card).ok()?;
            let mut mixer: *mut c_void = std::ptr::null_mut();

            unsafe {
                if snd_mixer_open(&mut mixer, 0) < 0 || mixer.is_null() {
                    return None;
                }
                if snd_mixer_attach(mixer, nome.as_ptr()) < 0 {
                    snd_mixer_close(mixer);
                    return None;
                }
                if snd_mixer_selem_register(mixer, std::ptr::null_mut(), std::ptr::null_mut()) < 0 {
                    snd_mixer_close(mixer);
                    return None;
                }
                if snd_mixer_load(mixer) < 0 {
                    snd_mixer_close(mixer);
                    return None;
                }

                let Some(elem) = Self::escolher(mixer) else {
                    snd_mixer_close(mixer);
                    return None;
                };
                Some(Self { mixer, elem })
            }
        }

        /// Prefere Master, depois PCM, depois Playback; senão o primeiro que
        /// tenha volume de reprodução.
        unsafe fn escolher(mixer: *mut c_void) -> Option<*mut c_void> {
            let mut primeiro: Option<*mut c_void> = None;
            let mut elem = snd_mixer_first_elem(mixer);

            while !elem.is_null() {
                if snd_mixer_selem_has_playback_volume(elem) > 0 {
                    if primeiro.is_none() {
                        primeiro = Some(elem);
                    }
                    let nome_ptr = snd_mixer_selem_get_name(elem);
                    if !nome_ptr.is_null() {
                        if let Ok(nome) = std::ffi::CStr::from_ptr(nome_ptr).to_str() {
                            if PREFERIDOS.contains(&nome) {
                                return Some(elem);
                            }
                        }
                    }
                }
                elem = snd_mixer_elem_next(elem);
            }
            primeiro
        }

        fn faixa(&self) -> Option<(i64, i64)> {
            let (mut min, mut max): (c_long, c_long) = (0, 0);
            let rc = unsafe {
                snd_mixer_selem_get_playback_volume_range(self.elem, &mut min, &mut max)
            };
            if rc < 0 || max <= min {
                return None;
            }
            Some((min as i64, max as i64))
        }
    }

    impl Mixer for AlsaMixer {
        fn get(&self) -> Option<u8> {
            let (min, max) = self.faixa()?;
            let mut valor: c_long = 0;
            let rc = unsafe {
                snd_mixer_selem_get_playback_volume(self.elem, CANAL_FRENTE_ESQ, &mut valor)
            };
            if rc < 0 {
                return None;
            }
            Some(converter(valor as i64, min, max))
        }

        fn set(&self, pct: u8) {
            let Some((min, max)) = self.faixa() else {
                return;
            };
            let alvo = min + (pct.min(100) as i64 * (max - min)) / 100;
            unsafe {
                snd_mixer_selem_set_playback_volume_all(self.elem, alvo as c_long);
            }
        }
    }

    fn converter(valor: i64, min: i64, max: i64) -> u8 {
        let faixa = max - min;
        if faixa <= 0 {
            return 0;
        }
        (((valor - min) as f64 / faixa as f64) * 100.0).round().clamp(0.0, 100.0) as u8
    }

    impl Drop for AlsaMixer {
        fn drop(&mut self) {
            if !self.mixer.is_null() {
                unsafe {
                    snd_mixer_close(self.mixer);
                }
            }
        }
    }
}

#[cfg(target_os = "linux")]
pub use alsa::AlsaMixer;

/// Mixer do `card0`, ou um dublê indisponível em plataformas sem ALSA.
pub fn abrir_mixer() -> MixerBox {
    #[cfg(target_os = "linux")]
    {
        if let Some(m) = AlsaMixer::abrir("hw:0") {
            return MixerBox::Alsa(RefCell::new(m));
        }
        if let Some(m) = AlsaMixer::abrir("default") {
            return MixerBox::Alsa(RefCell::new(m));
        }
    }
    MixerBox::Ausente
}

/// Mixer resolvido em tempo de execução.
///
/// Existe porque a escolha (`hw:0` ou `default`) só se sabe depois de tentar, e
/// o tipo precisa ser único para o `Volume` ser genérico mas o app guardar um só
/// valor.
pub enum MixerBox {
    #[cfg(target_os = "linux")]
    Alsa(RefCell<AlsaMixer>),
    Ausente,
}

impl Mixer for MixerBox {
    fn get(&self) -> Option<u8> {
        match self {
            #[cfg(target_os = "linux")]
            MixerBox::Alsa(m) => m.borrow().get(),
            MixerBox::Ausente => None,
        }
    }

    fn set(&self, pct: u8) {
        match self {
            #[cfg(target_os = "linux")]
            MixerBox::Alsa(m) => m.borrow().set(pct),
            MixerBox::Ausente => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// Mixer de mentira: guarda o valor e conta as escritas.
    struct Falso {
        valor: Cell<Option<u8>>,
        escritas: Cell<u32>,
    }

    impl Falso {
        fn novo(v: Option<u8>) -> Self {
            Self {
                valor: Cell::new(v),
                escritas: Cell::new(0),
            }
        }
    }

    impl Mixer for Falso {
        fn get(&self) -> Option<u8> {
            self.valor.get()
        }
        fn set(&self, pct: u8) {
            self.valor.set(Some(pct));
            self.escritas.set(self.escritas.get() + 1);
        }
    }

    fn dir_temporario(nome: &str) -> String {
        let p = std::env::temp_dir().join(format!(
            "giochi-volume-{nome}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&p).unwrap();
        p.to_string_lossy().into_owned()
    }

    #[test]
    fn le_o_volume_atual_do_hardware() {
        let v = Volume::novo(Falso::novo(Some(42)));
        assert!(v.disponivel());
        assert_eq!(v.atual(), 42);
    }

    #[test]
    fn escreve_no_mixer_a_cada_mudanca() {
        let m = Falso::novo(Some(50));
        let mut v = Volume::novo(&m);
        v.definir(55);
        assert_eq!(m.valor.get(), Some(55));
        assert_eq!(m.escritas.get(), 1);
    }

    #[test]
    fn limite_superior_e_aparado() {
        let mut v = Volume::novo(Falso::novo(Some(98)));
        assert_eq!(v.definir(200), 100, "valor acima de 100 vira 100");
    }

    #[test]
    fn sem_mixer_nao_escreve_em_lugar_nenhum() {
        let m = Falso::novo(None);
        let mut v = Volume::novo(&m);
        v.definir(70);
        assert_eq!(m.escritas.get(), 0);
        assert_eq!(v.atual(), 70, "a UI ainda mostra o valor escolhido");
    }

    #[test]
    fn nao_escreve_quando_o_valor_nao_muda() {
        // Arrastar o slider passando pelo mesmo valor não pode virar I/O.
        let m = Falso::novo(Some(50));
        let mut v = Volume::novo(&m);
        v.definir(50);
        assert_eq!(m.escritas.get(), 0);
    }

    #[test]
    fn repeticao_dispara_um_passo_ao_apertar() {
        let mut r = Repeticao::novo();
        assert_eq!(r.update(true, 0.016), 1);
    }

    #[test]
    fn repeticao_espera_o_atraso_antes_de_repetir() {
        let mut r = Repeticao::com_tempos(0.4, 0.1);
        assert_eq!(r.update(true, 0.016), 1, "primeiro passo imediato");
        // Dentro do atraso: nada.
        for _ in 0..20 {
            assert_eq!(r.update(true, 0.016), 0);
        }
    }

    #[test]
    fn repeticao_acelera_depois_do_atraso() {
        let mut r = Repeticao::com_tempos(0.2, 0.1);
        r.update(true, 0.016);
        let mut extras = 0;
        for _ in 0..60 {
            extras += r.update(true, 0.016);
        }
        // atraso 0,2 s = ~13 quadros; depois um passo a cada ~7 quadros
        // (0,1 s / 0,016). Em 60 quadros: ~7 repetições.
        assert!(
            (6..=9).contains(&extras),
            "esperava ~7 repetições em 0,96 s, veio {extras}"
        );
    }

    #[test]
    fn repeticao_para_ao_soltar() {
        let mut r = Repeticao::com_tempos(0.1, 0.05);
        r.update(true, 0.016);
        for _ in 0..20 {
            r.update(true, 0.016);
        }
        assert_eq!(r.update(false, 0.016), 0);
        assert_eq!(r.update(false, 0.016), 0, "solto não repete");
        // Apertar de novo recomeça com atraso cheio.
        assert_eq!(r.update(true, 0.016), 1);
    }

    #[test]
    fn sem_mixer_a_ui_ainda_funciona() {
        let mut v = Volume::novo(Falso::novo(None));
        assert!(!v.disponivel(), "sem mixer precisa se declarar indisponível");
        // Definir não explode e mantém o valor na tela.
        assert_eq!(v.definir(55), 55);
    }
}

/// Repetição do ajuste quando a tecla fica pressionada.
///
/// Sem isto, segurar volume dá um passo de 5 % e para — o usuário precisa
/// apertar 20 vezes para ir de 0 a 100. O atraso inicial evita que um toque
/// rápido vire dois passos.
pub struct Repeticao {
    atraso: f32,
    intervalo: f32,
    desde_aperto: f32,
    ate_proximo: f32,
    pressionado: bool,
}

impl Repeticao {
    /// 400 ms até começar a repetir; depois um passo a cada 120 ms.
    pub fn novo() -> Self {
        Self {
            atraso: 0.4,
            intervalo: 0.12,
            desde_aperto: 0.0,
            ate_proximo: 0.0,
            pressionado: false,
        }
    }

    #[cfg(test)]
    pub fn com_tempos(atraso: f32, intervalo: f32) -> Self {
        Self {
            atraso,
            intervalo,
            desde_aperto: 0.0,
            ate_proximo: 0.0,
            pressionado: false,
        }
    }

    /// Alimenta o estado da tecla; devolve quantos passos aplicar neste quadro.
    pub fn update(&mut self, pressionado: bool, delta: f32) -> i16 {
        let antes = self.pressionado;
        self.pressionado = pressionado;

        if pressionado && !antes {
            self.desde_aperto = 0.0;
            // Zero, não `atraso`: o atraso já é controlado por `desde_aperto`.
            // Somar os dois fazia a primeira repetição vir em 2× o atraso.
            self.ate_proximo = 0.0;
            return 1;
        }
        if !pressionado {
            self.desde_aperto = 0.0;
            self.ate_proximo = 0.0;
            return 0;
        }

        self.desde_aperto += delta.max(0.0);
        if self.desde_aperto < self.atraso {
            return 0;
        }
        self.ate_proximo -= delta.max(0.0);
        if self.ate_proximo <= 0.0 {
            self.ate_proximo = self.intervalo;
            return 1;
        }
        0
    }
}
