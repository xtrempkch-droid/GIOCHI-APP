//! Efeitos de partículas: fumaça, respingo, faísca, nitro, chuva, motes e marcas.
//!
//! ## Por que um sistema só, com sete tipos
//!
//! A primeira versão tinha duas listas separadas (fumaça e rastro de nitro) e
//! um teto de 220 partículas. Encher a cena de efeito com esse desenho exigiria
//! uma lista por efeito, cada uma com o seu laço de envelhecimento — sete cópias
//! da mesma regra, que envelhecem diferente. Aqui há **uma** lista, um laço de
//! integração e um campo `tipo` que decide cor, gravidade, arrasto e forma de
//! desenho.
//!
//! O desenho continua barato: as partículas de um mesmo tipo saem numa
//! `draw_instanced`, então sete tipos são **sete** chamadas de desenho — não
//! sete mil. Um tipo sem partícula viva não é desenhado.
//!
//! ## O teto é por tipo, e existe por dois motivos
//!
//! Memória do aparelho (964 MB no X35H) e previsibilidade do quadro: um quadro
//! com derrapagem longa não pode custar o dobro de um quadro parado. Quando o
//! teto estoura, a partícula mais **antiga** sai — a que já perdeu 90% da vida e
//! quase não aparece.
//!
//! ## Tudo aqui é testável sem GL
//!
//! Nada neste módulo conhece `RenderContext3D`. As matrizes de instância saem
//! prontas de [`Particulas::matrizes`], e é o que permite provar em teste que a
//! chuva fica perto da câmera, que a marca de pneu deita no chão e que o rastro
//! se estica ao longo da velocidade.

use giochi_engine_3d::math3d::{Color3D, Mat4, Vec3};
use giochi_neon_drift_core::corrida::{Corrida, Fase};

use crate::vetor::{comprimento, direcao_normalizada, escalar, produto_escalar, somar, subtrair};

// ════════════════════════════════════════════════════════════
// TIPOS
// ════════════════════════════════════════════════════════════

/// O que a partícula é. Decide cor, física e forma de desenho.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    /// Fumaça de pneu queimando na derrapagem.
    Fumaca,
    /// Água levantada pelo pneu no asfalto molhado, mesmo sem derrapar.
    Respingo,
    /// Faísca de batida ou de atrito lateral forte.
    Faisca,
    /// Rastro de nitro nos escapamentos.
    Nitro,
    /// Gota de chuva: risco longo, alinhado à velocidade relativa.
    Chuva,
    /// Poeira de neon flutuando na cidade — dá vida ao ar parado.
    Mote,
    /// Marca de pneu no asfalto, deitada no chão e persistente.
    Marca,
}

impl Tipo {
    /// Todos os tipos, para o laço de desenho.
    pub const TODOS: [Tipo; 7] = [
        Tipo::Marca,
        Tipo::Fumaca,
        Tipo::Respingo,
        Tipo::Faisca,
        Tipo::Nitro,
        Tipo::Chuva,
        Tipo::Mote,
    ];

    /// Nome estável, usado no diagnóstico `--json`.
    pub fn nome(self) -> &'static str {
        match self {
            Tipo::Fumaca => "fumaca",
            Tipo::Respingo => "respingo",
            Tipo::Faisca => "faisca",
            Tipo::Nitro => "nitro",
            Tipo::Chuva => "chuva",
            Tipo::Mote => "mote",
            Tipo::Marca => "marca",
        }
    }

    /// Cor do quad. O alfa é a opacidade que a partícula usa já **nascendo**:
    /// a vida restante multiplica este valor no desenho.
    pub fn cor(self) -> Color3D {
        match self {
            Tipo::Fumaca => Color3D::new(0.86, 0.87, 0.95, 0.30),
            Tipo::Respingo => Color3D::new(0.52, 0.74, 1.0, 0.34),
            Tipo::Faisca => Color3D::new(1.0, 0.74, 0.20, 0.92),
            Tipo::Nitro => Color3D::new(0.30, 0.92, 1.0, 0.40),
            Tipo::Chuva => Color3D::new(0.62, 0.74, 0.96, 0.26),
            Tipo::Mote => Color3D::new(0.45, 0.98, 0.90, 0.70),
            // Quase preta e translúcida: a marca **escurece** o asfalto. Opaca,
            // viraria um buraco preto com borda dura no meio do quadro.
            Tipo::Marca => Color3D::new(0.030, 0.026, 0.042, 0.55),
        }
    }

    /// Teto de partículas vivas deste tipo.
    pub fn teto(self) -> usize {
        match self {
            // A fumaça é o sinal visual do drift e a partícula mais cara (é
            // grande). 260 é o que cobre uma derrapagem longa sem encher a tela.
            Tipo::Fumaca => 300,
            Tipo::Respingo => 260,
            Tipo::Faisca => 170,
            Tipo::Nitro => 150,
            // A chuva é barata por unidade: um risco de 1×14 px. É o efeito que
            // mais muda a cena por pixel gasto.
            Tipo::Chuva => 700,
            Tipo::Mote => 140,
            Tipo::Marca => 240,
        }
    }

    /// `true` quando o quad deita no chão em vez de encarar a câmera.
    fn deitada(self) -> bool {
        matches!(self, Tipo::Marca)
    }
}

// ════════════════════════════════════════════════════════════
// PARTÍCULA
// ════════════════════════════════════════════════════════════

/// Uma partícula viva.
#[derive(Debug, Clone, Copy)]
pub struct Particula {
    pub pos: Vec3,
    pub vel: Vec3,
    pub vida: f32,
    pub vida_max: f32,
    /// Tamanho base do quad, em metros.
    pub tamanho: f32,
    /// Quanto o quad se alonga ao longo da própria velocidade, em segundos.
    ///
    /// Zero é billboard redondo (fumaça). `0,03` num risco a 30 m/s dá 1,9× de
    /// comprimento — é o que transforma um quad em gota de chuva.
    pub esticado: f32,
    /// Giro do quad deitado no chão, em radianos (só `Marca` usa).
    pub giro: f32,
    pub tipo: Tipo,
}

impl Particula {
    /// Fração de vida restante, 1 no nascimento e 0 na morte.
    pub fn vida_restante(&self) -> f32 {
        if self.vida_max <= 0.0 {
            0.0
        } else {
            (self.vida / self.vida_max).clamp(0.0, 1.0)
        }
    }

    /// Opacidade no desenho: a cor do tipo, com o alfa seguindo a vida.
    ///
    /// A fumaça **cresce** enquanto desbota (é o que faz parecer fumaça e não
    /// bolinha); a faísca faz o contrário, some rápido.
    pub fn alfa(&self) -> f32 {
        let c = self.tipo.cor();
        let fase = 1.0 - self.vida_restante();
        let fator = match self.tipo {
            // Entra rápido e sai devagar: sem o pico no começo, a fumaça some
            // antes de sair de perto do pneu.
            Tipo::Fumaca => (self.vida_restante() * 1.35).min(1.0),
            Tipo::Faisca | Tipo::Nitro => self.vida_restante().powf(0.6),
            _ => self.vida_restante(),
        };
        // O crescimento da fumaça também dilui a cor.
        let diluicao = if self.tipo == Tipo::Fumaca {
            1.0 - fase * 0.45
        } else {
            1.0
        };
        c.a * fator * diluicao
    }

    /// Escala do quad, em metros, no instante atual.
    pub fn escala(&self) -> f32 {
        match self.tipo {
            // A fumaça expande até 2,2× o tamanho de nascimento.
            Tipo::Fumaca => self.tamanho * (0.55 + 1.65 * (1.0 - self.vida_restante())),
            Tipo::Respingo => self.tamanho * (1.0 + 0.8 * (1.0 - self.vida_restante())),
            Tipo::Marca => self.tamanho,
            _ => self.tamanho,
        }
    }
}

// ════════════════════════════════════════════════════════════
// SISTEMA
// ════════════════════════════════════════════════════════════

/// Todas as partículas vivas do jogo.
pub struct Particulas {
    itens: Vec<Particula>,
    /// Estado do sorteio determinístico.
    semente: u32,
    /// Acumulador fracionário por emissor contínuo: a 75 Hz uma taxa de 40/s dá
    /// 0,53 partícula por quadro, e arredondar para baixo daria zero para sempre.
    acumulado: [f32; 5],
    /// Faíscas pedidas por uma batida, consumidas no próximo avanço.
    faiscas_pedidas: u32,
    /// Contagem por tipo no quadro corrente (diagnóstico).
    contagem: [u32; 7],
}

const INDICE_FUMACA: usize = 0;
const INDICE_RESPINGO: usize = 1;
const INDICE_CHUVA: usize = 2;
const INDICE_MOTE: usize = 3;
const INDICE_MARCA: usize = 4;

impl Particulas {
    pub fn nova() -> Self {
        Self {
            itens: Vec::with_capacity(1024),
            // Semente fixa: duas execuções com os mesmos passos desenham o mesmo
            // quadro. Sem isso a comparação PC × aparelho nunca fecharia.
            semente: 0x9e37_79b9,
            acumulado: [0.0; 5],
            faiscas_pedidas: 0,
            contagem: [0; 7],
        }
    }

    /// Quantas partículas vivas, no total.
    pub fn total(&self) -> usize {
        self.itens.len()
    }

    /// Quantas vivas de um tipo (usado pelos testes).
    #[cfg(test)]
    pub fn de(&self, tipo: Tipo) -> usize {
        self.itens.iter().filter(|p| p.tipo == tipo).count()
    }

    /// Contagem por tipo do último avanço, na ordem de [`Tipo::TODOS`].
    pub fn contagens(&self) -> &[u32; 7] {
        &self.contagem
    }

    /// Esvazia tudo (reinício de corrida).
    pub fn limpar(&mut self) {
        self.itens.clear();
    }

    /// Pede uma rajada de faíscas no carro do jogador (batida).
    pub fn pedir_faiscas(&mut self, quantas: u32) {
        self.faiscas_pedidas = (self.faiscas_pedidas + quantas).min(48);
    }

    // ── sorteio ──────────────────────────────────────────────

    /// xorshift32: determinístico, sem dependência e barato.
    fn sortear(&mut self) -> f32 {
        let mut x = self.semente;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.semente = x;
        (x >> 8) as f32 / 16_777_216.0
    }

    /// Sorteia em `[a, b)`.
    fn entre(&mut self, a: f32, b: f32) -> f32 {
        a + self.sortear() * (b - a)
    }

    // ── avanço ───────────────────────────────────────────────

    /// Integra, envelhece e emite.
    ///
    /// `olho` e `alvo` são a câmera do mundo: a chuva nasce numa caixa à frente
    /// dela, porque gerar chuva na volta inteira de 962 m seria 99% de partícula
    /// que ninguém vê.
    pub fn atualizar(&mut self, corrida: &Corrida, olho: Vec3, alvo: Vec3, dt: f32) {
        self.contagem = [0; 7];
        if dt <= 0.0 || dt > 0.25 {
            return;
        }
        self.integrar(dt);
        self.emitir_chuva(olho, alvo, corrida.carro.velocidade, dt);
        self.emitir_motes(olho, alvo, dt);

        if corrida.fase != Fase::Correndo {
            self.contabilizar();
            return;
        }
        self.emitir_carro(corrida, dt);
        self.contabilizar();
    }

    /// Só a física: move, derruba e descarta.
    fn integrar(&mut self, dt: f32) {
        for p in self.itens.iter_mut() {
            p.vida -= dt;
            p.pos = somar(p.pos, escalar(p.vel, dt));
            p.vel = match p.tipo {
                // Fumaça sobe e freia. Sem a subida ela parece areia.
                Tipo::Fumaca => somar(escalar(p.vel, 1.0 - 1.1 * dt), Vec3::new(0.0, 1.3 * dt, 0.0)),
                // Água e faísca caem de verdade.
                Tipo::Respingo => {
                    somar(escalar(p.vel, 1.0 - 0.9 * dt), Vec3::new(0.0, -9.8 * dt, 0.0))
                }
                Tipo::Faisca => {
                    somar(escalar(p.vel, 1.0 - 1.6 * dt), Vec3::new(0.0, -16.0 * dt, 0.0))
                }
                // Poeira de neon desce devagar, quase flutuando.
                Tipo::Mote => {
                    somar(escalar(p.vel, 1.0 - 0.5 * dt), Vec3::new(0.0, -0.35 * dt, 0.0))
                }
                // A gota não é acelerada: nascer já com a velocidade relativa é
                // o que faz a chuva ler como "o carro está rápido", e integrar
                // gravidade nela faria os riscos entortarem sozinhos.
                Tipo::Chuva | Tipo::Nitro | Tipo::Marca => p.vel,
            };
        }
        self.itens.retain(|p| p.vida > 0.0);
    }

    fn contabilizar(&mut self) {
        for p in &self.itens {
            if let Some(i) = Tipo::TODOS.iter().position(|t| *t == p.tipo) {
                self.contagem[i] += 1;
            }
        }
    }

    /// Insere respeitando o teto do tipo.
    fn empurrar(&mut self, p: Particula) {
        let tipo = p.tipo;
        if self.itens.iter().filter(|q| q.tipo == tipo).count() >= tipo.teto() {
            // Sai a mais antiga do mesmo tipo: é a que já desbotou. Pagar uma
            // varredura por partícula expulsa, e não por partícula emitida.
            if let Some(i) = self.itens.iter().position(|q| q.tipo == tipo) {
                self.itens.remove(i);
            }
        }
        self.itens.push(p);
    }

    /// Consome o acumulador e devolve quantas emitir neste passo.
    fn quantas(&mut self, indice: usize, taxa: f32, dt: f32) -> u32 {
        self.acumulado[indice] += taxa * dt;
        let n = self.acumulado[indice].floor();
        self.acumulado[indice] -= n;
        n.max(0.0).min(64.0) as u32
    }

    // ── emissores ────────────────────────────────────────────

    /// Chuva: nasce numa caixa à frente da câmera e cai para trás.
    fn emitir_chuva(&mut self, olho: Vec3, alvo: Vec3, velocidade: f32, dt: f32) {
        let frente = direcao_normalizada(subtrair(alvo, olho));
        let (direita, cima) = crate::vetor::base_billboard(frente);

        // A taxa cobre o teto no tempo de vida médio: 700 gotas / 0,55 s.
        let quantas = self.quantas(INDICE_CHUVA, 1_150.0, dt);
        for _ in 0..quantas {
            let ao_longo = self.entre(3.0, 74.0);
            let lateral = self.entre(-30.0, 30.0);
            let altura = self.entre(-5.0, 21.0);
            let vida = self.entre(0.40, 0.62);
            let tamanho = self.entre(0.035, 0.07);
            let pos = somar(
                somar(somar(olho, escalar(frente, ao_longo)), escalar(direita, lateral)),
                escalar(cima, altura),
            );
            // Velocidade relativa: a gota está quase parada no mundo e a câmera
            // avança. O risco é desenhado ao longo deste vetor.
            let arrasto = velocidade * 0.85;
            self.empurrar(Particula {
                pos,
                vel: somar(escalar(frente, -arrasto), Vec3::new(0.0, -26.0, 0.0)),
                vida,
                vida_max: 0.62,
                tamanho,
                esticado: 0.030,
                giro: 0.0,
                tipo: Tipo::Chuva,
            });
        }
    }

    /// Poeira de neon flutuando: nasce perto da câmera e vive muito.
    fn emitir_motes(&mut self, olho: Vec3, alvo: Vec3, dt: f32) {
        let frente = direcao_normalizada(subtrair(alvo, olho));
        let (direita, cima) = crate::vetor::base_billboard(frente);
        let quantas = self.quantas(INDICE_MOTE, 14.0, dt);
        for _ in 0..quantas {
            let ao_longo = self.entre(4.0, 46.0);
            let lateral = self.entre(-16.0, 16.0);
            let altura = self.entre(-3.0, 9.0);
            let vel = Vec3::new(
                self.entre(-0.5, 0.5),
                self.entre(0.05, 0.4),
                self.entre(-0.5, 0.5),
            );
            let vida = self.entre(2.5, 6.0);
            let tamanho = self.entre(0.05, 0.13);
            let pos = somar(
                somar(
                    somar(olho, escalar(frente, ao_longo)),
                    escalar(direita, lateral),
                ),
                escalar(cima, altura),
            );
            self.empurrar(Particula {
                pos,
                vel,
                vida,
                vida_max: 6.0,
                tamanho,
                esticado: 0.0,
                giro: 0.0,
                tipo: Tipo::Mote,
            });
        }
    }

    /// Tudo o que sai do carro do jogador.
    fn emitir_carro(&mut self, corrida: &Corrida, dt: f32) {
        let carro = &corrida.carro;
        let dir = carro.direcao();
        let (dx, dz) = (dir.x, dir.z);
        let (rx, rz) = (dz, -dx);
        let (px, pz) = (carro.pos.x, carro.pos.z);
        let velocidade = carro.velocidade.abs();
        let derrapagem = carro.derrapagem.abs();

        // ── fumaça de pneu ───────────────────────────────────
        if carro.derrapando {
            let forca = (derrapagem / 6.0).clamp(0.35, 2.2);
            let quantas = self.quantas(INDICE_FUMACA, 38.0 * forca, dt);
            for i in 0..quantas {
                let lado = if i % 2 == 0 { 1.0 } else { -1.0 };
                let atras = self.entre(1.3, 1.9);
                let altura = self.entre(0.16, 0.38);
                let espalha = self.entre(-1.1, 1.1);
                let sobe = self.entre(0.9, 1.9);
                let vida = self.entre(0.55, 0.95);
                let tamanho = self.entre(0.55, 0.95) * (0.8 + forca * 0.3);
                self.empurrar(Particula {
                    pos: Vec3::new(
                        px - dx * atras + rx * 0.88 * lado,
                        altura,
                        pz - dz * atras + rz * 0.88 * lado,
                    ),
                    vel: Vec3::new(
                        -dx * forca * 2.2 + rx * espalha,
                        sobe,
                        -dz * forca * 2.2 + rz * espalha,
                    ),
                    vida,
                    vida_max: vida,
                    tamanho,
                    esticado: 0.0,
                    giro: 0.0,
                    tipo: Tipo::Fumaca,
                });
            }

            // ── marca de pneu no asfalto ─────────────────────
            let giro = carro.angulo;
            let marcas = self.quantas(INDICE_MARCA, 46.0, dt);
            for i in 0..marcas {
                let lado = if i % 2 == 0 { 1.0 } else { -1.0 };
                let vida = self.entre(4.0, 7.0);
                self.empurrar(Particula {
                    pos: Vec3::new(
                        px - dx * 1.55 + rx * 0.82 * lado,
                        0.018,
                        pz - dz * 1.55 + rz * 0.82 * lado,
                    ),
                    vel: Vec3::ZERO,
                    vida,
                    vida_max: 7.0,
                    tamanho: 0.42,
                    esticado: 0.55,
                    giro,
                    tipo: Tipo::Marca,
                });
            }
        }

        // ── respingo de asfalto molhado ──────────────────────
        if velocidade > 14.0 {
            let forca = ((velocidade - 14.0) / 48.0).clamp(0.05, 1.0);
            let quantas = self.quantas(INDICE_RESPINGO, 80.0 * forca, dt);
            for i in 0..quantas {
                let lado = if i % 2 == 0 { 1.0 } else { -1.0 };
                let sobe = self.entre(1.4, 3.4);
                let desvia = self.entre(-1.4, 1.4);
                let vida = self.entre(0.22, 0.42);
                let tamanho = self.entre(0.10, 0.24);
                let empurra = 2.5 + forca * 5.0;
                self.empurrar(Particula {
                    pos: Vec3::new(
                        px - dx * 1.75 + rx * 0.8 * lado,
                        0.10,
                        pz - dz * 1.75 + rz * 0.8 * lado,
                    ),
                    vel: Vec3::new(
                        -dx * empurra + rx * desvia,
                        sobe,
                        -dz * empurra + rz * desvia,
                    ),
                    vida,
                    vida_max: 0.42,
                    tamanho,
                    esticado: 0.020,
                    giro: 0.0,
                    tipo: Tipo::Respingo,
                });
            }
        }

        // ── rastro de nitro ──────────────────────────────────
        if carro.nitro_usando {
            for _ in 0..4 {
                for lado in [-1.0f32, 1.0] {
                    let altura = self.entre(0.42, 0.72);
                    let empurra = self.entre(11.0, 17.0);
                    let desvia = self.entre(-0.1, 0.5);
                    let vida = self.entre(0.16, 0.30);
                    let tamanho = self.entre(0.34, 0.55);
                    self.empurrar(Particula {
                        pos: Vec3::new(
                            px - dx * 2.15 + rx * 0.46 * lado,
                            altura,
                            pz - dz * 2.15 + rz * 0.46 * lado,
                        ),
                        vel: Vec3::new(-dx * empurra, desvia, -dz * empurra),
                        vida,
                        vida_max: 0.30,
                        tamanho,
                        esticado: 0.014,
                        giro: 0.0,
                        tipo: Tipo::Nitro,
                    });
                }
            }
        }

        // ── faísca ───────────────────────────────────────────
        // Batida pedida de fora…
        let mut rajada = std::mem::take(&mut self.faiscas_pedidas);
        // …e atrito lateral forte, que solta faísca sozinho.
        if derrapagem > 9.0 {
            rajada += ((derrapagem - 9.0) as u32).min(6);
        }
        for _ in 0..rajada.min(40) {
            let lado_r = self.entre(-1.0, 1.0);
            let lado_f = self.entre(-1.0, 1.0);
            let lateral = self.entre(-7.0, 7.0);
            let ao_longo = self.entre(-3.0, 1.0);
            let sobe = self.entre(1.5, 6.0);
            let altura = self.entre(0.25, 0.75);
            let vida = self.entre(0.18, 0.45);
            let tamanho = self.entre(0.07, 0.16);
            self.empurrar(Particula {
                pos: Vec3::new(px + rx * lado_r, altura, pz + rz * lado_r),
                vel: Vec3::new(
                    rx * lateral + dx * ao_longo,
                    sobe,
                    rz * lateral + dz * ao_longo,
                ),
                vida,
                vida_max: 0.45,
                tamanho,
                esticado: 0.022,
                giro: 0.0,
                tipo: Tipo::Faisca,
            });
            let _ = lado_f;
        }
    }

    /// Quantas chamadas de desenho um tipo usa.
    ///
    /// `draw_instanced` pinta todas as instâncias com **uma** cor, então a única
    /// forma de a fumaça desbotar de verdade é separá-la em faixas de vida e
    /// desenhar cada faixa com o seu alfa. Dois baldes bastam: o de baixo com
    /// 40% de opacidade e o de cima com 100% lê como fumaça desbotando. Os
    /// outros tipos não precisam — a gota de chuva não perde opacidade em 0,5 s
    /// e a faísca some por tamanho, não por transparência.
    pub fn faixas(tipo: Tipo) -> usize {
        match tipo {
            Tipo::Fumaca => 2,
            _ => 1,
        }
    }

    /// Alfa médio de uma faixa de vida, para a cor da chamada de desenho.
    pub fn alfa_da_faixa(&self, tipo: Tipo, faixa: usize, total: usize) -> f32 {
        let mut soma = 0.0;
        let mut n = 0.0;
        for p in self.itens.iter().filter(|p| p.tipo == tipo && faixa_de(p, faixa, total)) {
            soma += p.alfa();
            n += 1.0;
        }
        if n > 0.0 {
            soma / n
        } else {
            0.0
        }
    }

    /// Empacota as matrizes de instância de uma faixa de um tipo.
    ///
    /// `direita`/`cima` vêm da câmera (billboard). As marcas ignoram a base e
    /// deitam no plano do chão.
    pub fn matrizes(
        &self,
        tipo: Tipo,
        faixa: usize,
        total: usize,
        direita: Vec3,
        cima: Vec3,
        saida: &mut Vec<Mat4>,
    ) {
        saida.clear();
        for p in self
            .itens
            .iter()
            .filter(|p| p.tipo == tipo && faixa_de(p, faixa, total))
        {
            saida.push(if p.tipo.deitada() {
                matriz_deitada(p.pos, p.giro, p.escala() * 0.62, p.escala() * 3.4)
            } else {
                matriz_esticada(p.pos, direita, cima, p.vel, p.escala(), p.esticado)
            })
        }
    }
}

/// `true` quando a partícula cai no balde `faixa` de `total` baldes de vida.
///
/// O balde 0 é o das **mais velhas** (vida restante baixa) e o último o das mais
/// novas — a ordem em que [`Particulas::matrizes`] é chamada pelo desenho vai do
/// transparente ao opaco, e o resultado não depende da ordem do `Vec`.
fn faixa_de(p: &Particula, faixa: usize, total: usize) -> bool {
    if total <= 1 {
        return true;
    }
    let v = (p.vida_restante() * total as f32) as usize;
    v.min(total - 1) == faixa
}

/// Luz sem sombreamento: a cor da malha é a cor final.
///
/// Partícula, marca de pneu e tudo o que é aditivo usam esta luz. Com a luz do
/// mundo (ambiente 0,62 + difusa) a fumaça branca saía cinza-escura no meio da
/// pista mal iluminada, e a marca de pneu desaparecia.
pub const LUZ_CHAPADA: giochi_engine_3d::DirectionalLight = giochi_engine_3d::DirectionalLight {
    direction: Vec3::new(0.0, 0.0, -1.0),
    color: Color3D::BLACK,
    ambient: 1.0,
    specular: 0.0,
    shininess: 1.0,
    _reserved: [0; 4],
};

// ════════════════════════════════════════════════════════════
// MATRIZES
// ════════════════════════════════════════════════════════════

/// Quad virado para a câmera, centrado em `pos`.
#[cfg(test)]
pub fn matriz_billboard(pos: Vec3, direita: Vec3, cima: Vec3, escala: f32) -> Mat4 {
    let mut m = Mat4::IDENTITY.to_cols_array();
    m[0] = direita.x * escala;
    m[1] = direita.y * escala;
    m[2] = direita.z * escala;
    m[4] = cima.x * escala;
    m[5] = cima.y * escala;
    m[6] = cima.z * escala;
    m[12] = pos.x;
    m[13] = pos.y;
    m[14] = pos.z;
    Mat4::from_cols_array(&m)
}

/// Quad deitado no chão, com o eixo longo apontando no rumo `giro`.
///
/// `giro` é o ângulo do carro (`0` aponta para +Z, crescendo no sentido horário
/// visto de cima), então o eixo longo é `(sin, 0, cos)` e o curto é
/// `(cos, 0, -sin)`.
pub fn matriz_deitada(pos: Vec3, giro: f32, largura: f32, comprimento: f32) -> Mat4 {
    let (s, c) = giro.sin_cos();
    // Zero e não identidade: os campos não escritos abaixo (`m[1]`, `m[5]`) são a
    // componente Y dos eixos, e deixá-los em 1,0 cisalha o quad para cima.
    let mut m = [0.0f32; 16];
    m[10] = 1.0;
    m[15] = 1.0;
    m[0] = c * largura;
    m[2] = -s * largura;
    // Eixo longo = **-frente**. Com +frente a normal do quad aponta para baixo
    // e, com o descarte de faces ligado, a marca de pneu some — sem erro, sem
    // aviso, e o sintoma é "o drift não deixa marca".
    m[4] = -s * comprimento;
    m[6] = -c * comprimento;
    m[12] = pos.x;
    m[13] = pos.y;
    m[14] = pos.z;
    Mat4::from_cols_array(&m)
}

/// Quad virado para a câmera e **esticado ao longo da própria velocidade**.
///
/// A velocidade é projetada na base do billboard, e o eixo longo do quad segue
/// essa projeção. É o que faz uma gota de chuva virar risco em vez de bolinha, e
/// o mesmo mecanismo dá o rastro do nitro e a faísca.
pub fn matriz_esticada(
    pos: Vec3,
    direita: Vec3,
    cima: Vec3,
    vel: Vec3,
    escala: f32,
    esticado: f32,
) -> Mat4 {
    let mut m = Mat4::IDENTITY.to_cols_array();
    let (lx, ly) = if esticado > 0.0 {
        let a = produto_escalar(vel, direita);
        let b = produto_escalar(vel, cima);
        let n = (a * a + b * b).sqrt();
        if n > 1e-3 {
            (a / n, b / n)
        } else {
            (0.0, 1.0)
        }
    } else {
        (0.0, 1.0)
    };
    // Eixo curto: perpendicular ao longo dentro do plano do billboard.
    //
    // O sinal negativo existe por causa do descarte de faces: com
    // `direita × cima` a normal do quad aponta **para longe** da câmera e, com o
    // culling ligado, toda partícula some — o quadro fica 3,9% diferente e não há
    // erro nenhum. Inverter o eixo curto espelha o retângulo (invisível, ele é
    // simétrico) e vira a normal para quem olha.
    let curto = somar(escalar(direita, ly), escalar(cima, -lx));
    let longo = somar(escalar(direita, lx), escalar(cima, ly));
    let comprimento = escala * (1.0 + comprimento(vel) * esticado);

    m[0] = curto.x * escala;
    m[1] = curto.y * escala;
    m[2] = curto.z * escala;
    m[4] = longo.x * comprimento;
    m[5] = longo.y * comprimento;
    m[6] = longo.z * comprimento;
    m[12] = pos.x;
    m[13] = pos.y;
    m[14] = pos.z;
    Mat4::from_cols_array(&m)
}

/// Base de billboard a partir de uma câmera (posição e alvo).
pub fn base_da_camera(olho: Vec3, alvo: Vec3) -> (Vec3, Vec3) {
    let frente = direcao_normalizada(subtrair(alvo, olho));
    crate::vetor::base_billboard(frente)
}

/// Normal de um triângulo, para teste de orientação de quad.
#[cfg(test)]
fn normal(cantos: &[Vec3; 3]) -> Vec3 {
    crate::vetor::produto_vetorial(
        subtrair(cantos[1], cantos[0]),
        subtrair(cantos[2], cantos[0]),
    )
}

#[cfg(test)]
mod testes {
    use super::*;
    use giochi_neon_drift_core::corrida::Corrida;

    fn cena_correndo() -> Corrida {
        let mut c = Corrida::nova();
        c.comecar();
        c
    }

    #[test]
    fn o_teto_de_cada_tipo_e_respeitado() {
        let mut p = Particulas::nova();
        for tipo in Tipo::TODOS {
            for i in 0..(tipo.teto() * 3) {
                p.empurrar(Particula {
                    pos: Vec3::new(i as f32, 0.0, 0.0),
                    vel: Vec3::ZERO,
                    vida: 10.0,
                    vida_max: 10.0,
                    tamanho: 1.0,
                    esticado: 0.0,
                    giro: 0.0,
                    tipo,
                });
            }
            assert_eq!(p.de(tipo), tipo.teto(), "{} estourou o teto", tipo.nome());
        }
    }

    #[test]
    fn a_chuva_fica_perto_da_camera() {
        let mut p = Particulas::nova();
        let olho = Vec3::new(100.0, 3.0, -400.0);
        let alvo = Vec3::new(100.0, 1.0, -390.0);
        for _ in 0..120 {
            p.atualizar(&cena_correndo(), olho, alvo, 1.0 / 75.0);
        }
        assert!(p.de(Tipo::Chuva) > 200, "pouca chuva: {}", p.de(Tipo::Chuva));
        for item in p.itens.iter().filter(|q| q.tipo == Tipo::Chuva) {
            let d = comprimento(crate::vetor::subtrair(item.pos, olho));
            assert!(d < 110.0, "gota a {d} m da câmera");
        }
    }

    #[test]
    fn a_marca_de_pneu_deita_no_chao_e_nao_encara_a_camera() {
        let mut p = Particulas::nova();
        p.empurrar(Particula {
            pos: Vec3::new(1.0, 0.02, 2.0),
            vel: Vec3::ZERO,
            vida: 5.0,
            vida_max: 5.0,
            tamanho: 0.4,
            esticado: 0.5,
            giro: 0.0,
            tipo: Tipo::Marca,
        });
        let mut saida = Vec::new();
        p.matrizes(
            Tipo::Marca,
            0,
            1,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            &mut saida,
        );
        let m = saida[0];
        // Colunas 0 e 1 sem componente em Y: o quad está deitado.
        assert!(m.data[1].abs() < 1e-6, "coluna 0 sobe: {}", m.data[1]);
        assert!(m.data[5].abs() < 1e-6, "coluna 1 sobe: {}", m.data[5]);
        assert!((m.data[13] - 0.02).abs() < 1e-6);
    }

    #[test]
    fn o_rastro_se_estica_ao_longo_da_velocidade() {
        let direita = Vec3::new(1.0, 0.0, 0.0);
        let cima = Vec3::new(0.0, 1.0, 0.0);
        // Velocidade na vertical: o eixo longo tem de ser o "cima" do billboard.
        let m = matriz_esticada(
            Vec3::ZERO,
            direita,
            cima,
            Vec3::new(0.0, 30.0, 0.0),
            1.0,
            0.03,
        );
        // comprimento = 1 * (1 + 30*0.03) = 1,9 na coluna 1.
        assert!((m.data[5] - 1.9).abs() < 1e-4, "comprimento {}", m.data[5]);
        // A largura é o tamanho base; quem cresce é o eixo longo.
        assert!((m.data[0].abs() - 1.0).abs() < 1e-4, "largura {}", m.data[0]);
    }

    #[test]
    fn sem_esticado_o_quad_fica_quadrado() {
        let m = matriz_esticada(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(50.0, 0.0, 0.0), 2.0, 0.0);
        assert!((m.data[0] - 2.0).abs() < 1e-4);
        assert!((m.data[5] - 2.0).abs() < 1e-4);
    }

    #[test]
    fn derrapar_emite_fumaca_e_marca() {
        let mut c = cena_correndo();
        c.carro.pos = c.pista.ponto_em(30.0);
        c.carro.angulo = c.pista.direcao_em(30.0).z.atan2(c.pista.direcao_em(30.0).x) - std::f32::consts::FRAC_PI_2;
        c.carro.angulo = 0.0;
        c.carro.derrapando = true;
        c.carro.derrapagem = 7.0;
        c.carro.velocidade = 40.0;

        let mut p = Particulas::nova();
        let olho = Vec3::new(0.0, 3.0, -8.0);
        let alvo = Vec3::new(0.0, 1.0, 4.0);
        // 90 quadros = 1,2 s de derrapagem. Com 40 quadros a contagem ficava
        // presa ao regime transitório e o teste media a taxa de emissão, não o
        // efeito — a asserção virava um número mágico que quebrava a cada ajuste.
        for _ in 0..90 {
            p.atualizar(&c, olho, alvo, 1.0 / 75.0);
        }
        assert!(p.de(Tipo::Fumaca) > 20, "fumaça {}", p.de(Tipo::Fumaca));
        assert!(p.de(Tipo::Marca) > 20, "marca {}", p.de(Tipo::Marca));
        // O regime permanente é taxa × vida média (80·0,54 · 0,32 ≈ 14).
        assert!(p.de(Tipo::Respingo) > 8, "respingo {}", p.de(Tipo::Respingo));
    }

    #[test]
    fn o_nitro_deixa_rastro_e_a_batida_solta_faisca() {
        let mut c = cena_correndo();
        c.carro.nitro_usando = true;
        let mut p = Particulas::nova();
        let olho = Vec3::new(0.0, 3.0, -8.0);
        let alvo = Vec3::new(0.0, 1.0, 4.0);
        for _ in 0..30 {
            p.atualizar(&c, olho, alvo, 1.0 / 75.0);
        }
        assert!(p.de(Tipo::Nitro) > 30, "rastro {}", p.de(Tipo::Nitro));

        p.pedir_faiscas(12);
        p.atualizar(&c, olho, alvo, 1.0 / 75.0);
        assert!(p.de(Tipo::Faisca) >= 12, "faíscas {}", p.de(Tipo::Faisca));
    }

    #[test]
    fn a_fumaca_cresce_e_desbota() {
        let p = Particula {
            pos: Vec3::ZERO,
            vel: Vec3::ZERO,
            vida: 1.0,
            vida_max: 1.0,
            tamanho: 1.0,
            esticado: 0.0,
            giro: 0.0,
            tipo: Tipo::Fumaca,
        };
        let mut velha = p;
        velha.vida = 0.1;
        assert!(velha.escala() > p.escala(), "a fumaça não cresceu");
        assert!(velha.alfa() < p.alfa(), "a fumaça não desbotou");
    }

    #[test]
    fn o_sorteio_e_deterministico() {
        let a: Vec<f32> = {
            let mut p = Particulas::nova();
            (0..8).map(|_| p.sortear()).collect()
        };
        let b: Vec<f32> = {
            let mut p = Particulas::nova();
            (0..8).map(|_| p.sortear()).collect()
        };
        assert_eq!(a, b, "a mesma semente tem de dar a mesma sequência");
        assert!(a.windows(2).any(|w| w[0] != w[1]), "sequência constante");
        assert!(a.iter().all(|v| (0.0..1.0).contains(v)), "fora de [0,1)");
    }

    #[test]
    fn a_cena_parada_nao_emite_fumaca() {
        let c = Corrida::nova(); // fase Titulo
        let mut p = Particulas::nova();
        for _ in 0..30 {
            p.atualizar(&c, Vec3::new(0.0, 3.0, -8.0), Vec3::new(0.0, 1.0, 4.0), 1.0 / 75.0);
        }
        assert_eq!(p.de(Tipo::Fumaca), 0);
        assert_eq!(p.de(Tipo::Marca), 0);
        // Mas a cidade continua com chuva e poeira.
        assert!(p.de(Tipo::Chuva) > 100);
        assert!(p.total() > 100);
    }

    #[test]
    fn nada_sobrevive_depois_da_vida() {
        let mut p = Particulas::nova();
        p.empurrar(Particula {
            pos: Vec3::ZERO,
            vel: Vec3::ZERO,
            vida: 0.05,
            vida_max: 0.05,
            tamanho: 1.0,
            esticado: 0.0,
            giro: 0.0,
            tipo: Tipo::Faisca,
        });
        for _ in 0..10 {
            p.integrar(1.0 / 75.0);
        }
        assert_eq!(p.de(Tipo::Faisca), 0);
    }

    #[test]
    fn a_base_da_camera_olha_para_o_alvo() {
        let (direita, cima) = base_da_camera(Vec3::new(0.0, 3.0, -8.0), Vec3::new(0.0, 1.0, 4.0));
        assert!(produto_escalar(direita, Vec3::UP).abs() < 1e-4);
        assert!(cima.y > 0.9 && cima.y < 1.0, "cima {cima:?}");
    }

    #[test]
    fn a_normal_do_quad_deitado_aponta_para_cima() {
        let m = matriz_deitada(Vec3::ZERO, 0.7, 1.0, 3.0);
        let a = Vec3::new(m.data[0], m.data[1], m.data[2]);
        let b = Vec3::new(m.data[4], m.data[5], m.data[6]);
        let n = normal(&[Vec3::ZERO, a, b]).normalized();
        // Sinal importa: normal para baixo + `cull_face` ligado = quad invisível.
        assert!(n.y > 0.99, "normal aponta para baixo: {n:?}");
    }
}
