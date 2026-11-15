//! Transporte: tocar, pausar, parar e volume — o estado que a barra mostra.
//!
//! Separado do reducer porque é um valor pequeno com invariantes próprio, e o
//! reducer precisa de um lugar só para guardar o volume. O volume é do **mixer**
//! (o app tem um só), não da faixa: trocar de música não muda o volume.

/// Estado de reprodução.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EstadoPlay {
    Parado,
    Tocando,
    Pausado,
}

impl EstadoPlay {
    /// Rótulo da barra de transporte.
    pub fn rotulo(self) -> &'static str {
        match self {
            EstadoPlay::Parado => "Parado",
            EstadoPlay::Tocando => "Tocando",
            EstadoPlay::Pausado => "Pausado",
        }
    }

    /// `true` quando o mixer está de fato emitindo som.
    pub fn emitindo(self) -> bool {
        matches!(self, EstadoPlay::Tocando)
    }
}

/// Estado do transporte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transporte {
    pub estado: EstadoPlay,
    /// Volume de 0 a 100, em passos de 5.
    pub volume: i32,
}

impl Transporte {
    pub const PASSO_VOLUME: i32 = 5;
    pub const VOLUME_MAX: i32 = 100;

    /// Volume inicial.
    ///
    /// 70 e não 100: um app que abre no volume máximo estoura o ouvido de quem
    /// está com fone, e o usuário não tem como prever isso antes de apertar A.
    pub const VOLUME_PADRAO: i32 = 70;

    pub fn novo(volume: i32) -> Transporte {
        Transporte {
            estado: EstadoPlay::Parado,
            volume: limitar_volume(volume),
        }
    }

    /// Soma `passos` de 5% e devolve o volume resultante.
    pub fn ajustar_volume(&mut self, passos: i8) -> i32 {
        self.volume = limitar_volume(self.volume + i32::from(passos) * Self::PASSO_VOLUME);
        self.volume
    }

    /// Fração de 0,0 a 1,0 para a barra de volume.
    pub fn fracao_do_volume(&self) -> f32 {
        self.volume as f32 / Self::VOLUME_MAX as f32
    }
}

/// Trava o volume na faixa aceita pelo mixer (`0..=100`).
pub fn limitar_volume(volume: i32) -> i32 {
    volume.clamp(0, Transporte::VOLUME_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_inicial_e_conservador() {
        let t = Transporte::novo(Transporte::VOLUME_PADRAO);
        assert_eq!(t.volume, 70);
        assert_eq!(t.estado, EstadoPlay::Parado);
        assert!(!t.estado.emitindo());
    }

    #[test]
    fn volume_anda_em_passos_de_cinco() {
        let mut t = Transporte::novo(50);
        assert_eq!(t.ajustar_volume(1), 55);
        assert_eq!(t.ajustar_volume(2), 65);
        assert_eq!(t.ajustar_volume(-3), 50);
    }

    #[test]
    fn volume_nao_sai_da_faixa() {
        let mut t = Transporte::novo(95);
        assert_eq!(t.ajustar_volume(5), 100);
        assert_eq!(t.ajustar_volume(1), 100, "100 é o teto");
        let mut t = Transporte::novo(5);
        assert_eq!(t.ajustar_volume(-5), 0);
        assert_eq!(t.ajustar_volume(-1), 0, "0 é o piso");
    }

    #[test]
    fn volume_absurdo_e_travado_na_abertura() {
        assert_eq!(Transporte::novo(999).volume, 100);
        assert_eq!(Transporte::novo(-5).volume, 0);
    }

    #[test]
    fn fracao_do_volume_vai_de_zero_a_um() {
        assert!((Transporte::novo(0).fracao_do_volume() - 0.0).abs() < 1e-6);
        assert!((Transporte::novo(100).fracao_do_volume() - 1.0).abs() < 1e-6);
        assert!((Transporte::novo(50).fracao_do_volume() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn rotulos_do_estado_sao_os_da_tela() {
        assert_eq!(EstadoPlay::Parado.rotulo(), "Parado");
        assert_eq!(EstadoPlay::Tocando.rotulo(), "Tocando");
        assert_eq!(EstadoPlay::Pausado.rotulo(), "Pausado");
        assert!(EstadoPlay::Tocando.emitindo());
        assert!(!EstadoPlay::Pausado.emitindo());
    }
}
