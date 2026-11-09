//! O circuito: uma curva fechada descrita por pontos de controle.
//!
//! ## Por que Catmull-Rom em laço fechado, e não "retas e arcos"
//!
//! A alternativa era compor a pista de peças (`reta 120`, `curva 90° raio 50`).
//! Ela é fácil de raciocinar e **difícil de fechar**: a soma dos ângulos tem de
//! dar 360° e os raios têm de casar, senão sobra um degrau no ponto de emenda —
//! que aparece na tela como um tranco no volante e um vão no asfalto.
//!
//! Com pontos de controle em laço, o fechamento é **de construção**: a curva
//! passa por todos e volta ao primeiro. O que se perde é o raio exato; ganha-se
//! um teste que mede a curvatura resultante e reprova se a pista ficar sem
//! curva fechada de verdade (ver `tem_curva_fechada`).

/// Ponto no plano da pista. `x` é leste, `z` é norte; `y` é altura e não entra
/// na pista — o asfalto é plano e a câmera é que dá a perspectiva.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ponto {
    /// Leste.
    pub x: f32,
    /// Norte.
    pub z: f32,
}

impl Ponto {
    /// Constrói um ponto.
    pub const fn new(x: f32, z: f32) -> Self {
        Self { x, z }
    }
    /// Origem.
    pub const ZERO: Self = Self::new(0.0, 0.0);

    /// Soma de vetores.
    pub fn mais(self, outro: Self) -> Self {
        Self::new(self.x + outro.x, self.z + outro.z)
    }
    /// Subtração de vetores.
    pub fn menos(self, outro: Self) -> Self {
        Self::new(self.x - outro.x, self.z - outro.z)
    }
    /// Multiplicação por escalar.
    pub fn vezes(self, k: f32) -> Self {
        Self::new(self.x * k, self.z * k)
    }
    /// Produto escalar.
    pub fn ponto(self, outro: Self) -> f32 {
        self.x * outro.x + self.z * outro.z
    }
    /// Comprimento.
    pub fn comprimento(self) -> f32 {
        self.ponto(self).sqrt()
    }
    /// Versão unitária. Vetor nulo devolve o próprio zero, e não `NaN` — uma
    /// normal `NaN` contamina toda a geometria do quadro.
    pub fn normalizado(self) -> Self {
        let n = self.comprimento();
        if n <= 1e-6 {
            Self::ZERO
        } else {
            self.vezes(1.0 / n)
        }
    }
    /// Vetor perpendicular (girado 90° para a direita de quem olha de cima).
    pub fn perpendicular(self) -> Self {
        Self::new(self.z, -self.x)
    }
    /// Distância a outro ponto.
    pub fn distancia(self, outro: Self) -> f32 {
        self.menos(outro).comprimento()
    }
    /// Interpolação linear.
    pub fn misturar(self, outro: Self, t: f32) -> Self {
        self.vezes(1.0 - t).mais(outro.vezes(t))
    }
}

/// Espaçamento nominal entre amostras da linha central, em metros.
pub const PASSO: f32 = 2.0;

/// Altura do perfil em `fracao` (0..1), por interpolação linear.
///
/// Função livre e não método de `Mapa` porque a `Pista` também precisa dela ao
/// ser construída por `com_relevo`, sem um `Mapa` em mãos (os testes montam
/// pistas sintéticas).
pub fn altura_do_perfil(relevo: &[(f32, f32)], fracao: f32) -> f32 {
    if relevo.is_empty() {
        return 0.0;
    }
    let f = fracao - fracao.floor();
    for par in relevo.windows(2) {
        let (a, ha) = par[0];
        let (b, hb) = par[1];
        if f >= a && f <= b {
            let t = if (b - a).abs() < 1e-6 {
                0.0
            } else {
                (f - a) / (b - a)
            };
            return ha + (hb - ha) * t;
        }
    }
    relevo.last().map(|(_, h)| *h).unwrap_or(0.0)
}

/// Uma amostra da linha central, já com o comprimento de arco acumulado.
#[derive(Debug, Clone, Copy)]
pub struct Amostra {
    /// Distância percorrida desde o início da volta.
    pub s: f32,
    /// Ponto na linha central.
    pub centro: Ponto,
    /// Direção de corrida (unitária).
    pub direcao: Ponto,
    /// Direita da pista (unitária).
    pub normal: Ponto,
    /// Metade da largura do asfalto.
    pub meia_largura: f32,
    /// Altura do asfalto neste ponto, em metros.
    ///
    /// A pista é 2D no plano e **não** é plana: o relevo do mapa entra aqui e é
    /// o que faz a câmera subir na serra e a cidade ficar lá embaixo. Sem este
    /// campo, "descida" seria um circuito plano com nome bonito.
    pub altura: f32,
    /// `true` dentro do túnel.
    pub tunel: bool,
}

impl Amostra {
    /// Ponto deslocado `lateral` metros para a direita da linha central.
    pub fn em(&self, lateral: f32) -> Ponto {
        self.centro.mais(self.normal.vezes(lateral))
    }
}

/// Um obstáculo leve posto na pista (cone, barreira de pneus).
#[derive(Debug, Clone, Copy)]
pub struct Obstaculo {
    /// Posição na volta.
    pub s: f32,
    /// Deslocamento lateral a partir da linha central.
    pub lateral: f32,
    /// Raio de colisão, em metros.
    pub raio: f32,
}

/// Uma rampa já resolvida em metros de pista.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RampaS {
    /// Onde o pé da rampa começa, em metros desde a linha.
    pub inicio: f32,
    /// Onde o topo termina, em metros. Depois disto o piso cai a pique.
    pub topo: f32,
    /// Altura do topo acima do asfalto, em metros.
    pub altura: f32,
}

impl RampaS {
    /// Inclinação da subida (metros de altura por metro de pista).
    pub fn inclinacao(&self) -> f32 {
        let comprimento = (self.topo - self.inicio).max(1e-3);
        self.altura / comprimento
    }

    /// Altura da rampa em `s`, ou zero fora dela.
    ///
    /// **Depois do topo é zero, não rampa descendo**: é esse corte seco que
    /// faz o carro sair do chão. Uma descida suave seria uma ladeira e o carro
    /// desceria grudado nela.
    pub fn altura_em(&self, s: f32) -> f32 {
        if s < self.inicio || s > self.topo {
            return 0.0;
        }
        let fracao = (s - self.inicio) / (self.topo - self.inicio).max(1e-3);
        self.altura * fracao
    }
}

/// Encaixa `alvo` no `s` da amostra mais próxima e devolve esse `s`.
fn encaixar_em_amostra(amostras: &[Amostra], alvo: f32) -> f32 {
    let mut melhor = amostras[0].s;
    let mut distancia = (melhor - alvo).abs();
    for a in amostras {
        let d = (a.s - alvo).abs();
        if d < distancia {
            distancia = d;
            melhor = a.s;
        }
    }
    melhor
}

/// O circuito completo.
#[derive(Debug, Clone)]
pub struct Pista {
    amostras: Vec<Amostra>,
    comprimento: f32,
    obstaculos: Vec<Obstaculo>,
    /// Índice (em `amostras`) onde começa a reta de largada.
    largada: usize,
    /// Rampas de salto, em metros e ordenadas.
    rampas: Vec<RampaS>,
}

impl Pista {
    /// Constrói o circuito do mapa padrão.
    pub fn nova() -> Self {
        Self::do_mapa(crate::mapa::PADRAO)
    }

    /// Constrói a partir de pontos de controle, larguras e trechos de túnel.
    ///
    /// Exposto para teste: dá para montar uma pista sintética (um círculo, por
    /// exemplo) e conferir a matemática sem depender do traçado do jogo.
    pub fn com_controles(controles: &[Ponto], larguras: &[f32], tuneis: &[usize]) -> Self {
        Self::com_relevo(controles, larguras, tuneis, &[])
    }

    /// Constrói a partir de um mapa: traçado, larguras, túneis, **relevo** e
    /// **rampas**.
    pub fn do_mapa(mapa: &crate::mapa::Mapa) -> Self {
        Self::com_relevo_e_rampas(mapa.controles, mapa.larguras, mapa.tuneis, mapa.relevo, mapa.rampas)
    }

    /// Constrói a partir de pontos de controle, larguras, túneis e perfil de
    /// elevação (`(fração da volta, metros)`).
    pub fn com_relevo(
        controles: &[Ponto],
        larguras: &[f32],
        tuneis: &[usize],
        relevo: &[(f32, f32)],
    ) -> Self {
        Self::com_relevo_e_rampas(controles, larguras, tuneis, relevo, &[])
    }

    /// Igual a [`Self::com_relevo`], e mais as rampas de salto.
    pub fn com_relevo_e_rampas(
        controles: &[Ponto],
        larguras: &[f32],
        tuneis: &[usize],
        relevo: &[(f32, f32)],
        rampas: &[crate::mapa::Rampa],
    ) -> Self {
        let n = controles.len();
        assert!(n >= 4, "um laço fechado precisa de ao menos 4 pontos");
        assert_eq!(larguras.len(), n, "uma largura por ponto de controle");

        // Quantos passos por segmento: o suficiente para que o passo fique perto
        // de `PASSO` mesmo no segmento mais longo.
        let mut amostras: Vec<Amostra> = Vec::with_capacity(n * 24);
        let mut acumulado = 0.0f32;
        let mut anterior = controles[0];

        for segmento in 0..n {
            let p0 = controles[(segmento + n - 1) % n];
            let p1 = controles[segmento];
            let p2 = controles[(segmento + 1) % n];
            let p3 = controles[(segmento + 2) % n];

            let passos = passos_do_segmento(p1, p2);
            let largura = larguras[segmento];
            let tunel = tuneis.contains(&segmento);

            for passo in 0..passos {
                let t = passo as f32 / passos as f32;
                let centro = catmull_rom(p0, p1, p2, p3, t);
                if passo == 0 && segmento == 0 {
                    anterior = centro;
                }
                acumulado += centro.distancia(anterior);
                anterior = centro;

                // Derivada analítica da Catmull-Rom: a direção sai exata em vez de
                // ser inferida da diferença entre amostras vizinhas, que no
                // primeiro passo ainda não existe.
                let direcao = catmull_rom_derivada(p0, p1, p2, p3, t).normalizado();
                amostras.push(Amostra {
                    s: acumulado,
                    centro,
                    direcao,
                    normal: direcao.perpendicular(),
                    meia_largura: largura,
                    altura: 0.0,
                    tunel,
                });
            }
        }

        // Fecha o laço: a última amostra aponta de volta para a primeira.
        let primeiro = amostras[0].centro;
        acumulado += anterior.distancia(primeiro);
        let direcao = primeiro.menos(anterior).normalizado();
        let meia_largura = amostras.last().map(|a| a.meia_largura).unwrap_or(9.0);
        amostras.push(Amostra {
            s: acumulado,
            centro: primeiro,
            direcao,
            normal: direcao.perpendicular(),
            meia_largura,
            altura: 0.0,
            tunel: false,
        });

        // O relevo é aplicado **depois** de o comprimento estar fechado: a
        // fração da volta só existe quando se sabe o total. Aplicar durante a
        // construção usaria um comprimento ainda parcial e o perfil sairia
        // esticado de um jeito que só aparece na tela.
        if !relevo.is_empty() {
            let comprimento = acumulado.max(1e-3);
            for amostra in amostras.iter_mut() {
                amostra.altura = altura_do_perfil(relevo, amostra.s / comprimento);
            }
        }

        // As rampas entram **depois** do relevo e com o comprimento já fechado,
        // pelo mesmo motivo: a fração da volta só existe com o total conhecido.
        //
        // O topo é encaixado na amostra mais próxima. Isso não é preciosismo:
        // o asfalto é desenhado ligando `altura` de amostras vizinhas, e se o
        // topo caísse no meio de um segmento o piso desceria suave em vez de
        // acabar em corte — o carro nunca sairia do chão, e o defeito seria
        // visível só na tela.
        let mut rampas_s: Vec<RampaS> = Vec::with_capacity(rampas.len());
        if !rampas.is_empty() {
            let total = acumulado.max(1e-3);
            for r in rampas {
                let inicio_bruto = r.fracao * total;
                let topo_bruto = inicio_bruto + r.comprimento;
                let inicio = encaixar_em_amostra(&amostras, inicio_bruto);
                let topo = encaixar_em_amostra(&amostras, topo_bruto).max(inicio + 1.0);
                rampas_s.push(RampaS {
                    inicio,
                    topo,
                    altura: r.altura.max(0.0),
                });
            }
            rampas_s.sort_by(|a, b| a.inicio.partial_cmp(&b.inicio).unwrap_or(std::cmp::Ordering::Equal));
            for amostra in amostras.iter_mut() {
                for rampa in &rampas_s {
                    amostra.altura += rampa.altura_em(amostra.s);
                }
            }
        }

        // A linha de largada fica no primeiro ponto de controle, que é o começo
        // da reta longa.
        let largada = 0;

        let obstaculos = obstaculos_padrao(&amostras, controles);
        Self {
            comprimento: acumulado,
            amostras,
            obstaculos,
            largada,
            rampas: rampas_s,
        }
    }

    /// Rampas do circuito, em metros.
    pub fn rampas(&self) -> &[RampaS] {
        &self.rampas
    }

    /// A rampa cujo topo fica em `(de, ate]`, se o carro cruzou a borda.
    ///
    /// A janela é **aberta no começo e fechada no fim** de propósito: em dois
    /// passos seguidos, `de` é o `s` anterior e `ate` o atual, então cada topo
    /// é cruzado exatamente uma vez.
    pub fn rampa_cruzada(&self, de: f32, ate: f32) -> Option<RampaS> {
        self.rampas.iter().copied().find(|r| {
            // Um carro que deu a volta no mesmo passo (salto entre o fim e o
            // começo) cruzaria tudo; nesse caso não afirma nada.
            ate > de && r.topo > de && r.topo <= ate
        })
    }

    /// Comprimento total da volta, em metros.
    pub fn comprimento(&self) -> f32 {
        self.comprimento
    }

    /// Todas as amostras, em ordem.
    pub fn amostras(&self) -> &[Amostra] {
        &self.amostras
    }

    /// Índice da amostra de largada.
    pub fn largada(&self) -> usize {
        self.largada
    }

    /// Obstáculos leves da pista.
    pub fn obstaculos(&self) -> &[Obstaculo] {
        &self.obstaculos
    }

    /// Normaliza um comprimento de arco para dentro da volta.
    pub fn normalizar_s(&self, s: f32) -> f32 {
        let mut v = s % self.comprimento;
        if v < 0.0 {
            v += self.comprimento;
        }
        v
    }

    /// Índice da amostra imediatamente antes de `s` (busca binária).
    fn indice_de(&self, s: f32) -> usize {
        let s = self.normalizar_s(s);
        match self
            .amostras
            .binary_search_by(|a| a.s.partial_cmp(&s).unwrap_or(std::cmp::Ordering::Equal))
        {
            Ok(i) => i.min(self.amostras.len().saturating_sub(2)),
            Err(i) => i.saturating_sub(1).min(self.amostras.len().saturating_sub(2)),
        }
    }

    /// Interpola duas amostras vizinhas em `s`.
    fn interpolar(&self, s: f32) -> (Amostra, Amostra, f32) {
        let i = self.indice_de(s);
        let a = self.amostras[i];
        let b = self.amostras[(i + 1).min(self.amostras.len() - 1)];
        let vao = (b.s - a.s).max(1e-4);
        let t = ((self.normalizar_s(s) - a.s) / vao).clamp(0.0, 1.0);
        (a, b, t)
    }

    /// Ponto da linha central em `s`.
    pub fn ponto_em(&self, s: f32) -> Ponto {
        let (a, b, t) = self.interpolar(s);
        a.centro.misturar(b.centro, t)
    }

    /// Direção de corrida em `s` (unitária).
    pub fn direcao_em(&self, s: f32) -> Ponto {
        let (a, b, t) = self.interpolar(s);
        a.direcao.misturar(b.direcao, t).normalizado()
    }

    /// Direita da pista em `s` (unitária).
    pub fn normal_em(&self, s: f32) -> Ponto {
        let (a, b, t) = self.interpolar(s);
        a.normal.misturar(b.normal, t).normalizado()
    }

    /// Metade da largura do asfalto em `s`.
    pub fn meia_largura_em(&self, s: f32) -> f32 {
        let (a, b, t) = self.interpolar(s);
        a.meia_largura + (b.meia_largura - a.meia_largura) * t
    }

    /// Altura do asfalto em `s`, em metros.
    pub fn altura_em(&self, s: f32) -> f32 {
        let (a, b, t) = self.interpolar(s);
        a.altura + (b.altura - a.altura) * t
    }

    /// Inclinação em `s`: metros de subida por metro percorrido.
    ///
    /// Positiva subindo, negativa descendo. É derivada das amostras vizinhas e
    /// não do perfil do mapa: quem consulta quer a inclinação **da pista**, e
    /// uma segunda fonte de verdade para o mesmo número envelheceria diferente.
    pub fn inclinacao_em(&self, s: f32) -> f32 {
        let passo = 4.0;
        let a = self.altura_em(s - passo);
        let b = self.altura_em(s + passo);
        (b - a) / (passo * 2.0)
    }

    /// `true` quando `s` cai dentro do túnel.
    pub fn em_tunel(&self, s: f32) -> bool {
        let (a, ..) = self.interpolar(s);
        a.tunel
    }

    /// Ponto deslocado `lateral` metros à direita da linha central em `s`.
    pub fn ponto_lateral(&self, s: f32, lateral: f32) -> Ponto {
        self.ponto_em(s).mais(self.normal_em(s).vezes(lateral))
    }

    /// Onde um ponto do mundo está em relação à pista.
    ///
    /// `dica` é o índice anterior: procurar numa janela em vez de na volta toda
    /// é o que mantém o custo constante por quadro. `None` em `dica` faz busca
    /// global, que é o caso do nascimento e do reposicionamento.
    pub fn localizar(&self, pos: Ponto, dica: Option<usize>) -> Localizacao {
        let total = self.amostras.len();
        // A janela de ±25 amostras ≈ ±25 m é maior que qualquer passo por quadro
        // (82 m/s a 75 Hz dá 1,1 m) e menor que a volta inteira — mas ela é
        // **circular**. Sem o módulo, cruzar a linha prendia a localização: a
        // dica ficava no fim do vetor, a janela não alcançava o começo, e o
        // carro era localizado 2 m depois da linha com 12 m de desvio lateral.
        // O resultado era o carro travado a 2,7 m/s empurrando a mureta.
        let janela: Vec<usize> = match dica {
            Some(i) => {
                let centro = i.min(total - 1) as isize;
                (-25isize..=25)
                    .map(|k| (centro + k).rem_euclid(total as isize) as usize)
                    .collect()
            }
            None => (0..total).collect(),
        };

        let mut melhor = janela[0];
        let mut melhor_d = f32::MAX;
        for &i in &janela {
            let d = self.amostras[i].centro.distancia(pos);
            if d < melhor_d {
                melhor_d = d;
                melhor = i;
            }
        }

        let a = self.amostras[melhor];
        let relativo = pos.menos(a.centro);
        // Projeta no eixo da pista para refinar o `s` entre duas amostras.
        let ao_longo = relativo.ponto(a.direcao).clamp(-PASSO, PASSO);
        let lateral = relativo.ponto(a.normal);
        // Distância **perpendicular**, não a distância à amostra mais próxima.
        //
        // As amostras ficam a ~2 m uma da outra, então medir até a mais próxima
        // dá até 1 m de erro numa reta perfeita — e como a borda é comparada com
        // a meia-largura, esse metro virava "o carro está fora da pista" em
        // pista estreita. Medido: 0,82 m de distância com o carro exatamente na
        // linha central.
        Localizacao {
            indice: melhor,
            s: self.normalizar_s(a.s + ao_longo),
            lateral,
            distancia: lateral.abs() + ao_longo.abs() * 0.05,
            meia_largura: a.meia_largura,
        }
    }

    /// Menor raio de curvatura da pista, em metros.
    ///
    /// Serve para provar que existe uma **curva fechada** de verdade: uma pista
    /// de arcos suaves tem raio mínimo alto e é entediante, e o pedido era
    /// "inclua uma curva fechada".
    pub fn raio_minimo(&self) -> f32 {
        let mut menor = f32::MAX;
        let n = self.amostras.len();
        for i in 0..n {
            let a = self.amostras[i];
            let b = self.amostras[(i + 3).min(n - 1)];
            let ds = (b.s - a.s).abs();
            // O fecho do laço cria um segmento curtíssimo no fim da lista; com
            // ele, `raio = ds/ângulo` dá metros de raio para uma curva de 50.
            if ds < 1.0 {
                continue;
            }
            let angulo = a.direcao.ponto(b.direcao).clamp(-1.0, 1.0).acos();
            if angulo > 1e-4 {
                let raio = ds / angulo;
                if raio < menor {
                    menor = raio;
                }
            }
        }
        if menor == f32::MAX {
            f32::MAX
        } else {
            menor
        }
    }

    /// `s` onde a pista faz a curva mais fechada.
    pub fn s_de_raio_minimo(&self) -> f32 {
        let mut melhor = (f32::MAX, 0.0f32);
        let n = self.amostras.len();
        for i in 0..n {
            let a = self.amostras[i];
            let b = self.amostras[(i + 3).min(n - 1)];
            let ds = (b.s - a.s).abs();
            if ds < 1.0 {
                continue;
            }
            let angulo = a.direcao.ponto(b.direcao).clamp(-1.0, 1.0).acos();
            if angulo <= 1e-4 {
                continue;
            }
            let raio = ds / angulo;
            if raio < melhor.0 {
                melhor = (raio, a.s);
            }
        }
        melhor.1
    }

    /// Faixa de `s` ocupada pelo túnel, se houver.
    pub fn faixa_do_tunel(&self) -> Option<(f32, f32)> {
        let mut inicio: Option<f32> = None;
        let mut fim = 0.0;
        for a in &self.amostras {
            match (a.tunel, inicio) {
                (true, None) => inicio = Some(a.s),
                (true, Some(_)) => fim = a.s,
                (false, Some(i)) if i < fim => break,
                _ => {}
            }
        }
        inicio.map(|i| (i, fim))
    }

    /// Folga até a borda do asfalto, em metros (negativa = dentro da pista).
    ///
    /// Percorre a volta inteira, sem dica: é consulta de montagem de cenário, não
    /// de quadro. Existe porque **um objeto do cenário não pode cair em cima de
    /// outro trecho da pista** — e num laço fechado isso acontece: os prédios do
    /// fim da volta nasciam em cima da reta de largada, e a câmera amanhecia
    /// dentro de um deles. Com o descarte de faces ligado o erro era invisível
    /// (via-se através da parede); desligado, o quadro inteiro virou uma parede
    /// cinza.
    pub fn folga_em(&self, pos: Ponto) -> f32 {
        let mut menor = f32::MAX;
        for a in &self.amostras {
            let d = a.centro.distancia(pos) - a.meia_largura;
            if d < menor {
                menor = d;
            }
        }
        menor
    }

    /// Maior meia-largura da pista — a praça de drift.
    pub fn largura_maxima(&self) -> f32 {
        self.amostras
            .iter()
            .map(|a| a.meia_largura)
            .fold(0.0f32, f32::max)
    }
}

/// Resultado de localizar um ponto do mundo na pista.
#[derive(Debug, Clone, Copy)]
pub struct Localizacao {
    /// Índice da amostra mais próxima.
    pub indice: usize,
    /// Posição na volta, em metros.
    pub s: f32,
    /// Deslocamento lateral (positivo à direita).
    pub lateral: f32,
    /// Distância à linha central.
    pub distancia: f32,
    /// Meia-largura naquele ponto.
    pub meia_largura: f32,
}

impl Localizacao {
    /// Quanto o carro está fora do asfalto, em metros (0 dentro).
    pub fn fora_por(&self) -> f32 {
        (self.distancia - self.meia_largura).max(0.0)
    }
}

/// Quantos passos usar num segmento, para o passo ficar perto de [`PASSO`].
fn passos_do_segmento(a: Ponto, b: Ponto) -> usize {
    let estimativa = (a.distancia(b) / PASSO * 1.6).ceil() as usize;
    estimativa.clamp(6, 120)
}

/// Avalia a Catmull-Rom centrípeta (α = 0,5) em `t ∈ [0,1]`.
///
/// Centrípeta e não uniforme porque a uniforme faz *laço* quando dois pontos de
/// controle ficam perto: o carro passaria por dentro da própria curva, e o
/// sintoma seria "a pista se cruza" sem nada apontar para a fórmula.
fn catmull_rom(p0: Ponto, p1: Ponto, p2: Ponto, p3: Ponto, t: f32) -> Ponto {
    let t0 = 0.0;
    let t1 = t0 + knot(p0, p1);
    let t2 = t1 + knot(p1, p2);
    let t3 = t2 + knot(p2, p3);
    let t = t1 + (t2 - t1) * t;

    let a1 = p0.vezes((t1 - t) / (t1 - t0)).mais(p1.vezes((t - t0) / (t1 - t0)));
    let a2 = p1.vezes((t2 - t) / (t2 - t1)).mais(p2.vezes((t - t1) / (t2 - t1)));
    let a3 = p2.vezes((t3 - t) / (t3 - t2)).mais(p3.vezes((t - t2) / (t3 - t2)));

    let b1 = a1.vezes((t2 - t) / (t2 - t0)).mais(a2.vezes((t - t0) / (t2 - t0)));
    let b2 = a2.vezes((t3 - t) / (t3 - t1)).mais(a3.vezes((t - t1) / (t3 - t1)));

    b1.vezes((t2 - t) / (t2 - t1)).mais(b2.vezes((t - t1) / (t2 - t1)))
}

/// Derivada analítica da Catmull-Rom centrípeta, por diferença finita central no
/// parâmetro de nó.
///
/// Diferença finita e não a derivada fechada: a fechada da centrípeta tem três
/// quocientes por termo e é fácil de escrever errado de um jeito que só aparece
/// numa curva específica. `h` pequeno erra na ordem de `h²`, que a 2 m de
/// amostragem é invisível.
fn catmull_rom_derivada(p0: Ponto, p1: Ponto, p2: Ponto, p3: Ponto, t: f32) -> Ponto {
    let h = 1e-3;
    let a = (t - h).max(0.0);
    let b = (t + h).min(1.0);
    let pa = catmull_rom(p0, p1, p2, p3, a);
    let pb = catmull_rom(p0, p1, p2, p3, b);
    pb.menos(pa)
}

/// Parâmetro de nó da Catmull-Rom centrípeta.
fn knot(a: Ponto, b: Ponto) -> f32 {
    a.distancia(b).sqrt().max(1e-3)
}

/// Cones e barreiras leves: encostar custa velocidade, não para o carro.
///
/// As posições saem do **traçado construído**, não de números escritos à mão. A
/// primeira versão usava `s = 168` fixo para a entrada da praça, e quando o
/// traçado mudou o obstáculo ficou 2 m fora do asfalto — um cone na grama, que
/// nunca colide com nada e não avisa. Aqui `s` vem da amostra mais próxima de
/// cada ponto de controle, então mover a pista move os cones.
fn obstaculos_padrao(amostras: &[Amostra], controles: &[Ponto]) -> Vec<Obstaculo> {
    // `s` da amostra mais próxima de um ponto de controle.
    let s_de = |alvo: Ponto| -> f32 {
        amostras
            .iter()
            .min_by(|a, b| {
                a.centro
                    .distancia(alvo)
                    .partial_cmp(&b.centro.distancia(alvo))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|a| a.s)
            .unwrap_or(0.0)
    };
    // Índices fixos, com saturação no fim: a posição dos cones é relativa ao
    // **traçado**, então mover a pista move os cones. Um mapa com menos de nove
    // pontos de controle usa o último em vez de estourar o índice.
    let ultimo = controles.len().saturating_sub(1);
    let praca = s_de(controles[3.min(ultimo)]);
    let grampo = s_de(controles[8.min(ultimo)]);

    vec![
        // Entrada da praça de drift: dois cones marcando a linha de fora.
        Obstaculo { s: praca - 22.0, lateral: 7.0, raio: 1.3 },
        Obstaculo { s: praca - 10.0, lateral: 8.0, raio: 1.3 },
        // Dentro da praça, delimitando a zona boa.
        Obstaculo { s: praca + 4.0, lateral: -9.0, raio: 1.2 },
        Obstaculo { s: praca + 18.0, lateral: 10.0, raio: 1.2 },
        // Antes do grampo, no lado de fora.
        Obstaculo { s: grampo - 30.0, lateral: -6.5, raio: 1.5 },
        // Saída do grampo.
        Obstaculo { s: grampo + 18.0, lateral: 6.5, raio: 1.5 },
    ]
}

/// Pista sintética com duas retas longas, para os testes de física.
///
/// Física não se testa numa pista com curva a cada 50 m: o carro bate na borda e
/// o que se mede passa a ser a colisão, não a aceleração. Aqui há 1000 m de reta
/// de cada lado, que é mais do que qualquer teste precisa percorrer.
#[cfg(test)]
pub(crate) fn pista_de_teste() -> Pista {
    use std::f32::consts::PI;
    let mut controles = Vec::new();
    for i in 0..=20 {
        controles.push(Ponto::new(0.0, i as f32 * 50.0));
    }
    for k in 1..=5 {
        let a = k as f32 / 6.0 * PI;
        controles.push(Ponto::new(30.0 - 30.0 * a.cos(), 1000.0 + 30.0 * a.sin()));
    }
    for i in (0..=20).rev() {
        controles.push(Ponto::new(60.0, i as f32 * 50.0));
    }
    for k in 1..=5 {
        let a = k as f32 / 6.0 * PI;
        controles.push(Ponto::new(30.0 + 30.0 * a.cos(), -30.0 * a.sin()));
    }
    let larguras = vec![10.0; controles.len()];
    Pista::com_controles(&controles, &larguras, &[])
}

/// A mesma pista de teste, com relevo.
///
/// Existe para separar gravidade de curvatura: na `BOAVISTA` as curvas da serra
/// tiram o carro da pista em três segundos, e o teste de "a descida acelera"
/// acabava medindo colisão. Aqui há 1000 m de reta com inclinação conhecida.
#[cfg(test)]
pub(crate) fn pista_de_teste_com_relevo(relevo: &[(f32, f32)]) -> Pista {
    use std::f32::consts::PI;
    let mut controles = Vec::new();
    for i in 0..=20 {
        controles.push(Ponto::new(0.0, i as f32 * 50.0));
    }
    for k in 1..=5 {
        let a = k as f32 / 6.0 * PI;
        controles.push(Ponto::new(30.0 - 30.0 * a.cos(), 1000.0 + 30.0 * a.sin()));
    }
    for i in (0..=20).rev() {
        controles.push(Ponto::new(60.0, i as f32 * 50.0));
    }
    for k in 1..=5 {
        let a = k as f32 / 6.0 * PI;
        controles.push(Ponto::new(30.0 + 30.0 * a.cos(), -30.0 * a.sin()));
    }
    let larguras = vec![10.0; controles.len()];
    Pista::com_relevo(&controles, &larguras, &[], relevo)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn pista() -> Pista {
        Pista::nova()
    }

    #[test]
    fn o_laco_fecha_sem_degrau() {
        // O último ponto amostrado e o primeiro têm de coincidir. Um degrau aqui
        // aparece na tela como um vão no asfalto e um tranco na direção.
        let p = pista();
        let primeiro = p.amostras()[0].centro;
        let ultimo = p.amostras().last().unwrap().centro;
        assert!(
            primeiro.distancia(ultimo) < 0.01,
            "o laço não fechou: {primeiro:?} vs {ultimo:?}"
        );
    }

    #[test]
    fn a_volta_tem_tamanho_plausivel() {
        let p = pista();
        assert!(
            (500.0..1200.0).contains(&p.comprimento()),
            "comprimento {} m fora do esperado para 2 voltas de ~40 s",
            p.comprimento()
        );
    }

    #[test]
    fn existe_curva_fechada_de_verdade() {
        // O pedido era "inclua uma curva fechada". Raio mínimo alto = pista de
        // arcos suaves, entediante. 35 m é o grampo do traçado atual.
        let raio = pista().raio_minimo();
        assert!(raio < 40.0, "raio mínimo {raio:.1} m: não há curva fechada");
    }

    #[test]
    fn existe_uma_praca_larga_para_drift() {
        let p = pista();
        assert!(
            p.largura_maxima() >= 16.0,
            "a maior meia-largura é {:.1} m; drift precisa de espaço",
            p.largura_maxima()
        );
    }

    #[test]
    fn existe_tunel_e_ele_nao_da_a_volta_toda() {
        let (inicio, fim) = pista().faixa_do_tunel().expect("a pista tem túnel");
        assert!(fim > inicio, "faixa de túnel vazia: {inicio}..{fim}");
        let trecho = fim - inicio;
        assert!(
            (30.0..260.0).contains(&trecho),
            "túnel de {trecho:.0} m: curto demais para notar, longo demais para iluminar"
        );
    }

    #[test]
    fn existe_reta_longa_para_nitro() {
        // Trecho sem curvatura por pelo menos 120 m seguidos — é a reta em que o
        // nitro faz diferença.
        let p = pista();
        let amostras = p.amostras();
        // "Reta" aqui é o trecho em que o raio de curvatura passa de 150 m:
        // uma Catmull-Rom entre pontos quase colineares **sempre** curva um
        // pouco, então exigir ângulo zero não acha nada.
        let mut corrente = 0.0f32;
        let mut maior = 0.0f32;
        for i in 0..amostras.len().saturating_sub(4) {
            let a = amostras[i];
            let b = amostras[i + 3];
            let ds = (b.s - a.s).abs().max(1e-3);
            let angulo = a.direcao.ponto(b.direcao).clamp(-1.0, 1.0).acos();
            let raio = if angulo > 1e-5 { ds / angulo } else { f32::MAX };
            if raio > 150.0 {
                corrente += amostras[i + 1].s - a.s;
                maior = maior.max(corrente);
            } else {
                corrente = 0.0;
            }
        }
        assert!(maior > 100.0, "maior reta tem {maior:.0} m; nitro precisa de mais");
    }

    #[test]
    fn amostras_ficam_igualmente_espacadas() {
        let p = pista();
        let passos: Vec<f32> = p.amostras().windows(2).map(|w| w[1].s - w[0].s).collect();
        let maior = passos.iter().cloned().fold(0.0f32, f32::max);
        let menor = passos.iter().cloned().fold(f32::MAX, f32::min);
        assert!(menor > 0.5, "passo mínimo {menor:.2} m: amostra repetida");
        assert!(maior < 6.0, "passo máximo {maior:.2} m: buraco na pista");
    }

    #[test]
    fn localizar_devolve_o_lugar_certo_na_linha_central() {
        let p = pista();
        for s in [0.0, 37.0, 150.0, 260.0, 400.0, 610.0] {
            let alvo = p.ponto_em(s);
            let achado = p.localizar(alvo, None);
            assert!(
                (achado.s - s).abs() < 3.0,
                "s={s} localizado como {:.1}",
                achado.s
            );
            assert!(achado.distancia < 0.6, "distância {:.2} m na própria linha", achado.distancia);
        }
    }

    #[test]
    fn localizar_com_dica_acha_o_mesmo_ponto_que_a_busca_global() {
        let p = pista();
        let s = 300.0;
        let pos = p.ponto_lateral(s, 4.0);
        let global = p.localizar(pos, None);
        let dica = p.localizar(pos, Some(global.indice));
        assert!((global.s - dica.s).abs() < 1.0);
        assert!((global.lateral - dica.lateral).abs() < 0.5);
    }

    #[test]
    fn lateral_positivo_fica_a_direita_da_direcao() {
        let p = pista();
        let s = 120.0;
        let centro = p.ponto_em(s);
        let direita = p.ponto_lateral(s, 5.0);
        let produto = p.normal_em(s).ponto(direita.menos(centro));
        assert!(produto > 4.9, "normal não aponta para o lado do deslocamento");
    }

    #[test]
    fn fora_da_pista_e_medido_em_metros() {
        let p = pista();
        let s = 100.0;
        let meia = p.meia_largura_em(s);
        let dentro = p.localizar(p.ponto_lateral(s, meia - 1.0), None);
        assert_eq!(dentro.fora_por(), 0.0);
        let fora = p.localizar(p.ponto_lateral(s, meia + 6.0), None);
        assert!((fora.fora_por() - 6.0).abs() < 0.8, "fora_por={}", fora.fora_por());
    }

    #[test]
    fn obstaculos_estao_dentro_do_asfalto() {
        let p = pista();
        for o in p.obstaculos() {
            let meia = p.meia_largura_em(o.s);
            assert!(
                o.lateral.abs() + o.raio <= meia,
                "obstáculo em s={} lateral={} não cabe na meia-largura {}",
                o.s,
                o.lateral,
                meia
            );
        }
    }

    #[test]
    fn normalizar_s_da_a_volta() {
        let p = pista();
        let L = p.comprimento();
        assert!((p.normalizar_s(L + 10.0) - 10.0).abs() < 0.01);
        assert!((p.normalizar_s(-10.0) - (L - 10.0)).abs() < 0.01);
    }

    #[test]
    fn pista_sintetica_de_raio_conhecido_tem_raio_minimo_proximo() {
        // Um círculo de raio 50: a medida de curvatura tem de dizer ~50.
        let n = 16;
        let controles: Vec<Ponto> = (0..n)
            .map(|i| {
                let a = i as f32 / n as f32 * std::f32::consts::TAU;
                Ponto::new(50.0 * a.cos(), 50.0 * a.sin())
            })
            .collect();
        let larguras = vec![8.0; n];
        let p = Pista::com_controles(&controles, &larguras, &[]);
        let raio = p.raio_minimo();
        assert!((42.0..58.0).contains(&raio), "raio medido {raio:.1} para um círculo de 50");
        let esperado = std::f32::consts::TAU * 50.0;
        assert!(
            (p.comprimento() - esperado).abs() < esperado * 0.05,
            "comprimento {:.1} para um círculo de perímetro {esperado:.1}",
            p.comprimento()
        );
    }

    /// Curvatura acumulada em `janela` metros a partir de `s`, em radianos.
    fn curvatura_em(p: &Pista, s: f32, janela: f32) -> f32 {
        let a = p.direcao_em(s);
        let b = p.direcao_em(s + janela);
        a.ponto(b).clamp(-1.0, 1.0).acos()
    }

    #[test]
    fn a_pista_tem_o_que_o_jogo_promete() {
        // O pedido do jogo é explícito: **reta longa para o nitro**, **área
        // ampla para drift**, **curva fechada** e **túnel iluminado**. Essas
        // quatro coisas são o desenho da pista, e um ajuste de ponto de controle
        // pode apagar qualquer uma sem quebrar mais nada — o sintoma seria o
        // jogador procurando uma curva que não existe.
        let p = pista();

        // Reta longa: 120 m com menos de 0,1 rad de curvatura acumulada.
        let mut maior_reta = 0.0f32;
        let mut corrida = 0.0f32;
        let mut s = 0.0;
        while s < p.comprimento() {
            if curvatura_em(&p, s, 40.0) < 0.12 {
                corrida += 10.0;
                maior_reta = maior_reta.max(corrida);
            } else {
                corrida = 0.0;
            }
            s += 10.0;
        }
        assert!(
            maior_reta >= 120.0,
            "a maior reta tem {maior_reta:.0} m; o nitro precisa de 120 m ou mais"
        );

        // Curva fechada: varredura de 45 m com mais de 1,9 rad de mudança de
        // rumo — raio médio abaixo de 24 m. É a definição que o jogador sente:
        // a curva em que é preciso largar o acelerador de verdade.
        //
        // A varredura é global de propósito: `raio_minimo()` olha três amostras
        // (~6 m) e é dominado por ruído de amostragem, então ele serve para
        // achar *onde*, não para provar *quanto*.
        let mut mais_fechada = 0.0f32;
        let mut onde = 0.0f32;
        let mut s = 0.0;
        while s < p.comprimento() {
            let c = curvatura_em(&p, s, 45.0);
            if c > mais_fechada {
                mais_fechada = c;
                onde = s;
            }
            s += 5.0;
        }
        let raio = 45.0 / mais_fechada;
        assert!(
            mais_fechada > 1.9,
            "a curva mais fechada fica em {onde:.0} m e tem {mais_fechada:.2} rad \
             em 45 m (raio {raio:.0} m) — não é curva fechada"
        );

        // Área ampla para drift: 19 m de meia-largura contra 9,5 na reta.
        assert!(
            p.largura_maxima() > 17.0,
            "a praça de drift tem só {:.1} m de meia-largura",
            p.largura_maxima()
        );

        // Túnel: uma faixa contínua de pelo menos 30 m.
        let (inicio, fim) = p
            .faixa_do_tunel()
            .expect("a pista não tem túnel nenhum");
        assert!(
            fim - inicio >= 30.0,
            "o túnel tem {:.0} m — curto demais para dar a sensação de dentro",
            fim - inicio
        );

        // E o túnel fica longe da largada, senão o jogador começa no escuro.
        assert!(
            inicio > 300.0,
            "o túnel começa a {inicio:.0} m da largada; o começo tem de ser na reta"
        );
        println!(
            "reta {maior_reta:.0} m · curva fechada {mais_fechada:.2} rad em {onde:.0} m \
             (raio {raio:.0} m) · túnel {:.0}..{:.0} m · largura máx {:.1} m",
            inicio,
            fim,
            p.largura_maxima()
        );
    }
}
