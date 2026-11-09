//! Som sintetizado em runtime: motor, drift e nitro.
//!
//! ## Por que não há `.wav` nenhum
//!
//! Três motivos, e os três já custaram caro neste projeto:
//!
//! 1. **Empacotamento** — um arquivo de áudio é mais um caminho que pode faltar
//!    no aparelho, e o sintoma seria silêncio, indistinguível de "o mixer não
//!    abriu".
//! 2. **Licença** — um som de motor baixado da internet é exatamente o tipo de
//!    conteúdo protegido que o pedido proíbe.
//! 3. **Memória** — o RK3326 tem 964 MB e o jogo divide com o sistema; um laço
//!    de 2 s de motor em 22 kHz seria 88 KB por som, e três sons disso.
//!
//! Sintetizar custa ~300 amostras por quadro a 75 Hz: nada. E o resultado é
//! original por construção.
//!
//! ## O que sai
//!
//! * **motor** — onda dente-de-serra com frequência ligada ao RPM, mais ruído
//!   filtrado; o dente-de-serra dá a aspereza que o ouvido lê como motor.
//! * **drift** — ruído com passa-banda ressonante, ligado ao deslize. Contínuo
//!   enquanto derrapa, que era o pedido ("som de derrapagem contínuo").
//! * **nitro** — varredura ascendente com ruído, em cima do motor.
//!
//! Todos os pesos vêm de `EstadoSom`, então o áudio não conhece o jogo.

use std::f32::consts::TAU;

use sdl2::audio::{AudioQueue, AudioSpecDesired};

/// Taxa de amostragem. 22 kHz é o suficiente para motor e ruído e gasta metade
/// da memória de 44,1 kHz — num jogo que roda com 964 MB totais isso conta.
pub const TAXA: i32 = 22_050;

/// Estado do som num quadro, calculado pelo jogo.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct EstadoSom {
    /// Rotação 0..1 dentro da marcha.
    pub rpm: f32,
    /// Marcha 1..6.
    pub marcha: u8,
    /// Acelerador apertado.
    pub acelerando: bool,
    /// Deslize lateral em m/s (0 = sem drift).
    pub deslize: f32,
    /// Nitro em uso.
    pub nitro: bool,
    /// Velocidade em km/h.
    pub kmh: f32,
    /// `false` nas telas de título/resultado: só o silêncio.
    pub ativo: bool,
}

/// Sintetizador. A parte de geração é pura; só `enfileirar` fala com o SDL.
pub struct Som {
    fila: Option<AudioQueue<i16>>,
    /// Fase do motor, 0..1.
    fase_motor: f32,
    /// Fase da varredura de nitro, 0..1.
    fase_nitro: f32,
    /// Fase do oscilador de ruído filtrado do drift.
    /// Fase do ruído de fundo. Guardada para o som não recomeçar a cada
    /// quadro — hoje só o gerador de vento a lê.
    #[allow(dead_code)]
    fase_ruido: f32,
    /// Ruído filtrado do drift (estado do passa-banda).
    filtro: f32,
    /// Gerador pseudoaleatório determinístico (xorshift).
    semente: u32,
    /// Amostras que faltam gerar para acompanhar o relógio.
    pendentes: f32,
    /// Ganho geral, 0..1 (silêncio quando o áudio não abriu).
    ganho: f32,
}

impl Som {
    /// Abre o dispositivo de áudio. Sem áudio, o jogo continua e fica mudo.
    ///
    /// Falhar aqui **não** é erro fatal: imagem sem placa, aparelho sem mixer,
    /// CI sem dispositivo. O aviso sai no log e `ganho` fica em zero.
    pub fn iniciar(plataforma: &giochi_engine::platform::sdl::Platform) -> Self {
        let mut som = Self::sem_dispositivo();
        let Ok(subsistema) = plataforma.audio_subsystem() else {
            eprintln!("som: subsistema de áudio indisponível — o jogo segue mudo");
            return som;
        };
        let desejado = AudioSpecDesired {
            freq: Some(TAXA),
            channels: Some(1),
            samples: Some(512),
        };
        match AudioQueue::<i16>::open_queue(&subsistema, None, &desejado) {
            Ok(fila) => {
                fila.resume();
                som.fila = Some(fila);
                som.ganho = 0.55;
                eprintln!("som: motor, drift e nitro sintetizados a {TAXA} Hz");
            }
            Err(erro) => {
                eprintln!("som: não abri o dispositivo ({erro}) — o jogo segue mudo");
            }
        }
        som
    }

    /// Sintetizador sem dispositivo (CI, `--sem-som`, teste).
    pub fn sem_dispositivo() -> Self {
        Self {
            fila: None,
            fase_motor: 0.0,
            fase_nitro: 0.0,
            fase_ruido: 0.0,
            filtro: 0.0,
            semente: 0x1234_5678,
            pendentes: 0.0,
            ganho: 0.0,
        }
    }

    /// `true` quando há dispositivo e ganho.
    pub fn ativo(&self) -> bool {
        self.fila.is_some() && self.ganho > 0.0
    }

    /// Avança o som `dt` segundos e enfileira as amostras correspondentes.
    pub fn atualizar(&mut self, estado: &EstadoSom, dt: f32) {
        if !self.ativo() {
            return;
        }
        if self.fila.as_ref().is_some_and(|f| f.size() > (TAXA as u32) / 2) {
            // Mais de meio segundo na fila: o aparelho está atrasado, e empurrar
            // mais só aumentaria a latência.
            return;
        }

        // Acumula em fração de amostra: a 75 Hz dá 294 amostras por quadro, e
        // descartar a fração a cada quadro produziria um estalo de 3 Hz.
        self.pendentes += dt.clamp(0.0, 0.1) * TAXA as f32;
        let quantas = self.pendentes.floor() as usize;
        if quantas == 0 {
            return;
        }
        self.pendentes -= quantas as f32;

        let mut buffer = vec![0i16; quantas];
        self.gerar(estado, &mut buffer);
        if let Some(fila) = &self.fila {
            // Erro ao enfileirar (dispositivo sumiu) não para o jogo: só emudece.
            if fila.queue_audio(&buffer).is_err() {
                self.ganho = 0.0;
            }
        }
    }

    /// Gera `saida.len()` amostras. Pura em relação ao SDL: é o que o teste usa.
    pub fn gerar(&mut self, estado: &EstadoSom, saida: &mut [i16]) {
        for amostra in saida.iter_mut() {
            let valor = self.amostra(estado);
            *amostra = (valor.clamp(-1.0, 1.0) * 32_000.0 * self.ganho) as i16;
        }
    }

    /// Uma amostra do mix.
    fn amostra(&mut self, estado: &EstadoSom) -> f32 {
        if !estado.ativo {
            // Tela de título/resultado: silêncio, mas as fases continuam
            // andando para não haver clique ao voltar.
            self.fase_motor = (self.fase_motor + 0.001) % 1.0;
            return 0.0;
        }

        // ── motor ────────────────────────────────────────────────────
        // Frequência: marcha baixa com rpm baixo dá ~55 Hz (marcha lenta); a
        // 6ª a 0,9 de rpm dá ~230 Hz. O ouvido lê isso como motor subindo.
        let base = 55.0 + estado.marcha as f32 * 12.0;
        let frequencia = base + estado.rpm * 120.0;
        self.fase_motor = (self.fase_motor + frequencia / TAXA as f32) % 1.0;
        // Dente-de-serra: 2·fase-1. A aspereza é o que faz parecer motor em vez
        // de um bipe.
        let dente = self.fase_motor * 2.0 - 1.0;
        let carga = if estado.acelerando { 0.55 } else { 0.30 };
        // Ruído de escape, sempre presente e proporcional à carga.
        let ruido_motor = self.ruido() * 0.10 * carga;
        let mut mix = dente * carga * 0.55 + ruido_motor;

        // ── drift ────────────────────────────────────────────────────
        // Ruído passa-banda: `filtro` é um integrador de primeira ordem sobre o
        // ruído, o que já tira o chiado agudo e deixa o "rrrr" do pneu.
        if estado.deslize > 0.5 {
            let intensidade = (estado.deslize / 10.0).clamp(0.0, 1.0);
            let cru = self.ruido();
            self.filtro += (cru - self.filtro) * 0.35;
            let banda = self.filtro - cru * 0.5;
            mix += banda * 0.55 * intensidade;
        } else {
            self.filtro *= 0.9;
        }

        // ── nitro ────────────────────────────────────────────────────
        if estado.nitro {
            // Varredura: a fase acelera enquanto o nitro está ligado.
            self.fase_nitro = (self.fase_nitro + (420.0 + self.fase_nitro * 900.0) / TAXA as f32) % 1.0;
            let varredura = (self.fase_nitro * TAU).sin();
            mix += varredura * 0.30 + self.ruido() * 0.18;
        } else {
            self.fase_nitro = 0.0;
        }

        // ── vento ────────────────────────────────────────────────────
        // Proporcional à velocidade: sem ele, a 200 km/h o carro soa parado.
        let vento = (estado.kmh / 240.0).clamp(0.0, 1.0);
        mix += self.ruido() * 0.06 * vento;

        mix * 0.5
    }

    /// Ruído branco determinístico em -1..1.
    fn ruido(&mut self) -> f32 {
        // xorshift32: barato e reprodutível. `rand` seria uma dependência inteira
        // para seis linhas.
        self.semente ^= self.semente << 13;
        self.semente ^= self.semente >> 17;
        self.semente ^= self.semente << 5;
        (self.semente as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    /// Silencia o mix (pausa, troca de tela).
    #[allow(dead_code)] // usado pelo `--sem-som`
    pub fn emudecer(&mut self) {
        self.ganho = 0.0;
        if let Some(fila) = &self.fila {
            fila.clear();
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn estado_padrao() -> EstadoSom {
        EstadoSom {
            rpm: 0.5,
            marcha: 3,
            acelerando: true,
            deslize: 0.0,
            nitro: false,
            kmh: 120.0,
            ativo: true,
        }
    }

    fn amostras(estado: &EstadoSom, n: usize) -> Vec<i16> {
        let mut som = Som::sem_dispositivo();
        som.ganho = 0.55;
        let mut buffer = vec![0i16; n];
        som.gerar(estado, &mut buffer);
        buffer
    }

    #[test]
    fn sem_ativo_o_som_e_silencio() {
        let estado = EstadoSom {
            ativo: false,
            ..estado_padrao()
        };
        for amostra in amostras(&estado, 512) {
            assert_eq!(amostra, 0);
        }
    }

    #[test]
    fn com_ativo_o_som_nao_e_silencio() {
        let buffer = amostras(&estado_padrao(), 1024);
        let pico = buffer.iter().map(|a| a.abs()).max().unwrap_or(0);
        assert!(pico > 1000, "pico {pico}: o motor não está soando");
    }

    #[test]
    fn nenhuma_amostra_estoura_a_faixa() {
        // Um mix somado sem clamp vira estouro e o áudio sai como estalo alto.
        for deslize in [0.0, 6.0, 20.0] {
            let estado = EstadoSom {
                deslize,
                nitro: true,
                acelerando: true,
                rpm: 1.0,
                ..estado_padrao()
            };
            let mut som = Som::sem_dispositivo();
            som.ganho = 1.0;
            let mut buffer = vec![0i16; 4096];
            som.gerar(&estado, &mut buffer);
            for a in &buffer {
                assert!(*a > -32_000 && *a < 32_000, "amostra {a} saturou");
            }
        }
    }

    #[test]
    fn o_nitro_muda_o_som() {
        let sem = amostras(&estado_padrao(), 2048);
        let com = amostras(
            &EstadoSom {
                nitro: true,
                ..estado_padrao()
            },
            2048,
        );
        let diferenca: f32 = sem
            .iter()
            .zip(com.iter())
            .map(|(a, b)| ((a - b) as f32).abs())
            .sum();
        assert!(
            diferenca > 10_000.0,
            "o nitro não mudou nada no áudio (diferença {diferenca:.0})"
        );
    }

    #[test]
    fn o_drift_muda_o_som() {
        let sem = amostras(&estado_padrao(), 2048);
        let com = amostras(
            &EstadoSom {
                deslize: 9.0,
                ..estado_padrao()
            },
            2048,
        );
        let diferenca: f32 = sem
            .iter()
            .zip(com.iter())
            .map(|(a, b)| ((a - b) as f32).abs())
            .sum();
        assert!(
            diferenca > 10_000.0,
            "o drift não mudou nada no áudio (diferença {diferenca:.0})"
        );
    }

    #[test]
    fn mais_rpm_sobe_a_frequencia() {
        // Mede cruzamentos de zero por amostra: é a definição de frequência e
        // não depende de conhecer a fórmula.
        let contar = |rpm: f32| {
            let estado = EstadoSom {
                rpm,
                marcha: 6,
                deslize: 0.0,
                nitro: false,
                kmh: 0.0,
                ..estado_padrao()
            };
            let buffer = amostras(&estado, 4096);
            buffer
                .windows(2)
                .filter(|par| (par[0] < 0) != (par[1] < 0))
                .count()
        };
        let baixo = contar(0.05);
        let alto = contar(0.95);
        assert!(
            alto > baixo + 20,
            "rpm baixo {baixo} cruzamentos, rpm alto {alto}: o motor não sobe de tom"
        );
    }

    #[test]
    fn a_fila_de_amostras_acompanha_o_relogio() {
        // 1 s de jogo tem de pedir ~TAXA amostras, sem sobrar nem faltar.
        let mut som = Som::sem_dispositivo();
        som.ganho = 0.5;
        let estado = estado_padrao();
        let dt = 1.0 / 75.0;
        let mut geradas = 0usize;
        for _ in 0..75 {
            som.pendentes += dt * TAXA as f32;
            let quantas = som.pendentes.floor() as usize;
            som.pendentes -= quantas as f32;
            let mut buffer = vec![0i16; quantas];
            som.gerar(&estado, &mut buffer);
            geradas += quantas;
        }
        assert!(
            (geradas as i32 - TAXA).abs() <= 2,
            "1 s gerou {geradas} amostras, esperado {TAXA}"
        );
    }

    #[test]
    fn emudecer_zera_o_ganho() {
        let mut som = Som::sem_dispositivo();
        som.ganho = 0.5;
        som.emudecer();
        assert_eq!(som.ganho, 0.0);
        assert!(!som.ativo());
    }

    #[test]
    fn o_ruido_e_deterministico() {
        // Reprodutibilidade: dois `Som` iguais produzem as mesmas amostras, o que
        // permite comparar quadros e depurar.
        let a = amostras(&estado_padrao(), 256);
        let b = amostras(&estado_padrao(), 256);
        assert_eq!(a, b);
    }
}
