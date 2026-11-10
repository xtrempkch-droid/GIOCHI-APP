//! Piloto automático: um jogador de mentira, com limites de gente.
//!
//! ## Por que existe
//!
//! "A dificuldade está boa" é opinião. Este módulo joga a partida sozinho, com
//! **limites de reação humanos**, e o `--balanceamento` lê o resultado: quantas
//! ondas passaram, quantas vidas sobraram, quanto tempo levou. Três parâmetros de
//! dificuldade viram três números comparáveis.
//!
//! ## Os limites são o ponto
//!
//! Um piloto que mira instantaneamente joga melhor que qualquer pessoa e mede
//! uma dificuldade que ninguém vai encontrar. Os três limites abaixo são o que
//! separa "o jogo é ganhável por um humano" de "o jogo é ganhável por um
//! `atan2`":
//!
//! - **giro limitado** ([`GIRO_POR_SEGUNDO`]): a câmera não teleporta. É o custo
//!   real de mirar num cubo do outro lado do asteroide.
//! - **tempo de reação** ([`REACAO`]): um alvo novo não é engajado no mesmo
//!   quadro em que nasce.
//! - **erro de mira** ([`TOLERANCIA`]): o piloto atira dentro de um cone, não no
//!   pixel. A folga de mira da arma já é 10 % do cubo; aqui é o equivalente
//!   angular.
//!
//! ## Determinístico, e por isso comparável
//!
//! Não há sorteio: o piloto escolhe sempre a **ameaça mais próxima do asteroide**
//! (a que vai bater primeiro), e a decisão é função do estado. Duas execuções do
//! mesmo comando dão o mesmo resultado — o que permite comparar duas dificuldades
//! sem atribuir a diferença ao acaso.

use crate::gameplay::camera::OrbitalCamera;
use giochi_engine_3d::math3d::Vec3;

/// Quanto o piloto gira a câmera por segundo, em radianos.
///
/// 2,4 rad/s ≈ 137°/s: uma volta completa em 2,6 s. É mais rápido que o
/// analógico do aparelho (2,6 rad/s de deflexão máxima) e bem mais lento que
/// "apontar direto" — de propósito, para o piloto não ser melhor que o jogador.
pub const GIRO_POR_SEGUNDO: f32 = 2.4;

/// Tempo entre ver um alvo novo e poder atirar nele, em segundos.
pub const REACAO: f32 = 0.22;

/// Tolerância angular para atirar, em radianos.
///
/// 0,10 rad ≈ 5,7°. A 15 m isso é 1,5 m de desvio — o cubo tem 0,8 de aresta e a
/// arma dá 10 % de folga, então o piloto às vezes erra, como um humano.
pub const TOLERANCIA: f32 = 0.10;

/// O que o piloto precisa saber do mundo para decidir.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Panorama {
    /// Onde a câmera está (o olho).
    pub olho: Vec3,
    /// Para onde ela olha (o alvo do `look_at`).
    pub alvo: Vec3,
    /// Quantos segundos se passaram desde que o alvo atual foi escolhido.
    pub tempo_no_alvo: f32,
}

/// Um cubo no ar, do ponto de vista do piloto.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ameaca {
    /// Posição no mundo.
    pub posicao: Vec3,
    /// Distância ao asteroide — quem está mais perto bate primeiro.
    pub distancia_ao_centro: f32,
}

/// A decisão de um quadro.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Decisao {
    /// Quanto girar em `yaw`, em radianos, neste quadro.
    pub yaw: f32,
    /// Quanto girar em `pitch`, em radianos, neste quadro.
    pub pitch: f32,
    /// Atirar agora?
    pub atirar: bool,
}

fn angulo_entre(a: Vec3, b: Vec3) -> f32 {
    let produto = a.x * b.x + a.y * b.y + a.z * b.z;
    let norma =
        (a.x * a.x + a.y * a.y + a.z * a.z).sqrt() * (b.x * b.x + b.y * b.y + b.z * b.z).sqrt();
    if norma <= 1e-6 {
        return std::f32::consts::PI;
    }
    (produto / norma).clamp(-1.0, 1.0).acos()
}

fn direcao_de(panorama: &Panorama) -> Vec3 {
    let frente = Vec3::new(
        panorama.alvo.x - panorama.olho.x,
        panorama.alvo.y - panorama.olho.y,
        panorama.alvo.z - panorama.olho.z,
    );
    let norma = (frente.x * frente.x + frente.y * frente.y + frente.z * frente.z).sqrt();
    if norma <= 1e-6 {
        return Vec3::new(0.0, 0.0, -1.0);
    }
    Vec3::new(frente.x / norma, frente.y / norma, frente.z / norma)
}

/// Escolhe o alvo: o cubo **mais perto do asteroide**, que é o que vai bater
/// primeiro. Desempate pela distância ao olho, para a escolha ser determinística.
pub fn escolher_alvo(panorama: &Panorama, ameacas: &[Ameaca]) -> Option<usize> {
    let mut melhor: Option<(usize, f32, f32)> = None;
    for (indice, ameaca) in ameacas.iter().enumerate() {
        if !ameaca.distancia_ao_centro.is_finite() || !ameaca.posicao.is_finite() {
            continue;
        }
        let dx = ameaca.posicao.x - panorama.olho.x;
        let dy = ameaca.posicao.y - panorama.olho.y;
        let dz = ameaca.posicao.z - panorama.olho.z;
        let ao_olho = (dx * dx + dy * dy + dz * dz).sqrt();
        let chave = (ameaca.distancia_ao_centro, ao_olho);
        match melhor {
            Some((_, melhor_perto, melhor_olho))
                if (chave.0 > melhor_perto)
                    || ((chave.0 - melhor_perto).abs() < 1e-3 && chave.1 >= melhor_olho) => {}
            _ => melhor = Some((indice, chave.0, chave.1)),
        }
    }
    melhor.map(|(indice, _, _)| indice)
}

/// Decide o que fazer neste quadro.
///
/// `dt` é o passo da simulação; o giro devolvido já vem multiplicado por ele, no
/// formato que a câmera orbital consome.
pub fn decidir(panorama: &Panorama, ameacas: &[Ameaca], dt: f32) -> Decisao {
    let parado = Decisao {
        yaw: 0.0,
        pitch: 0.0,
        atirar: false,
    };
    if !dt.is_finite() || dt <= 0.0 {
        return parado;
    }
    let Some(indice) = escolher_alvo(panorama, ameacas) else {
        return parado;
    };
    let alvo = ameacas[indice].posicao;
    let frente = direcao_de(panorama);

    let para_alvo = Vec3::new(
        alvo.x - panorama.olho.x,
        alvo.y - panorama.olho.y,
        alvo.z - panorama.olho.z,
    );
    let norma =
        (para_alvo.x * para_alvo.x + para_alvo.y * para_alvo.y + para_alvo.z * para_alvo.z).sqrt();
    if norma <= 1e-6 {
        return parado;
    }
    let para_alvo = Vec3::new(
        para_alvo.x / norma,
        para_alvo.y / norma,
        para_alvo.z / norma,
    );

    let erro = angulo_entre(frente, para_alvo);
    let passo = GIRO_POR_SEGUNDO * dt;

    // Componentes do erro nos dois eixos da câmera. `yaw` é o giro em torno de Y
    // (produto com `UP`); `pitch` é a diferença de altura, aproximada pelo
    // arco-seno da componente vertical — suficiente para um piloto que corrige a
    // cada quadro.
    let yaw_desejado = {
        let eixo = Vec3::new(frente.z, 0.0, -frente.x);
        let seno = eixo.x * para_alvo.x + eixo.y * para_alvo.y + eixo.z * para_alvo.z;
        let cosseno = frente.x * para_alvo.x + frente.z * para_alvo.z;
        seno.atan2(cosseno)
    };
    // O sinal do `pitch` é **invertido** em relação à intuição: `camera.pitch` é
    // a elevação do *olho* acima do alvo, então levantar o olho faz a câmera
    // olhar mais para baixo. Para mirar num alvo mais alto é preciso **baixar** o
    // olho. A primeira versão somava direto e o piloto subia até o limite de 80°
    // olhando para o chão, sem nunca atirar (medido: 0 abates em cinco partidas).
    let pitch_desejado = frente.y.asin() - para_alvo.y.asin();

    let limite = |valor: f32| valor.clamp(-passo, passo);

    Decisao {
        yaw: limite(yaw_desejado),
        pitch: limite(pitch_desejado),
        // O tempo de reação é o que impede o piloto de acertar tudo no primeiro
        // quadro em que o cubo nasce.
        atirar: erro <= TOLERANCIA && panorama.tempo_no_alvo >= REACAO,
    }
}

/// Panorama a partir da câmera orbital e do relógio do piloto.
pub fn panorama_da_camera(camera: &OrbitalCamera, tempo_no_alvo: f32) -> Panorama {
    let distancia = camera.distancia_segura();
    let pitch = camera.pitch_seguro();
    let cos_pitch = pitch.cos();
    let olho = Vec3::new(
        camera.target.x + distancia * cos_pitch * camera.yaw.sin(),
        camera.target.y + distancia * pitch.sin(),
        camera.target.z + distancia * cos_pitch * camera.yaw.cos(),
    );
    Panorama {
        olho,
        alvo: camera.target,
        tempo_no_alvo,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panorama(olho: Vec3, alvo: Vec3, tempo: f32) -> Panorama {
        Panorama {
            olho,
            alvo,
            tempo_no_alvo: tempo,
        }
    }

    fn ameaca(x: f32, z: f32) -> Ameaca {
        let posicao = Vec3::new(x, 0.0, z);
        let distancia_ao_centro = (x * x + z * z).sqrt();
        Ameaca {
            posicao,
            distancia_ao_centro,
        }
    }

    #[test]
    fn sem_ameaca_o_piloto_nao_faz_nada() {
        let p = panorama(Vec3::new(0.0, 0.0, 9.0), Vec3::ZERO, 5.0);
        let decisao = decidir(&p, &[], 1.0 / 60.0);
        assert_eq!(decisao.yaw, 0.0);
        assert_eq!(decisao.pitch, 0.0);
        assert!(!decisao.atirar);
    }

    #[test]
    fn o_alvo_e_o_que_vai_bater_primeiro() {
        // O cubo em (0, 3) está a 3 do centro; o em (0, -9) a 9. O piloto tem de
        // escolher o primeiro, mesmo que o segundo esteja mais perto do olho.
        let p = panorama(Vec3::new(0.0, 0.0, 9.0), Vec3::ZERO, 5.0);
        let ameacas = [ameaca(0.0, -9.0), ameaca(0.0, 3.0)];
        assert_eq!(escolher_alvo(&p, &ameacas), Some(1));
    }

    #[test]
    fn o_piloto_gira_para_o_alvo_e_nao_teleporta() {
        // O alvo está a 90° da frente; num quadro o piloto só pode girar
        // `GIRO_POR_SEGUNDO * dt`.
        let p = panorama(Vec3::new(0.0, 0.0, 9.0), Vec3::ZERO, 5.0);
        let ameacas = [ameaca(12.0, 0.0)];
        let dt = 1.0 / 60.0;
        let decisao = decidir(&p, &ameacas, dt);
        let maximo = GIRO_POR_SEGUNDO * dt;
        assert!(
            decisao.yaw.abs() <= maximo + 1e-6,
            "giro de {:.4} num quadro, máximo {maximo:.4}",
            decisao.yaw.abs()
        );
        assert!(
            decisao.yaw.abs() > maximo * 0.9,
            "o piloto tinha de girar no limite, e girou {:.4}",
            decisao.yaw.abs()
        );
    }

    #[test]
    fn o_piloto_atira_quando_esta_alinhado_e_com_tempo_de_reacao() {
        let p = panorama(Vec3::new(0.0, 0.0, 9.0), Vec3::ZERO, REACAO + 0.1);
        // Exatamente no eixo de visão.
        let ameacas = [ameaca(0.0, 0.0)];
        // `(0,0)` está na origem, que é o alvo: o piloto olha para lá.
        assert!(decidir(&p, &ameacas, 1.0 / 60.0).atirar);

        // Sem tempo de reação, o mesmo alinhamento não atira.
        let sem_reacao = panorama(Vec3::new(0.0, 0.0, 9.0), Vec3::ZERO, 0.0);
        assert!(
            !decidir(&sem_reacao, &ameacas, 1.0 / 60.0).atirar,
            "atirou sem esperar o tempo de reação"
        );
    }

    #[test]
    fn fora_da_tolerancia_o_piloto_nao_atira() {
        // Um cubo a 60° do eixo de visão, com tempo de sobra: atirar seria
        // desperdício, e o piloto é medido pelo que acerta.
        let p = panorama(Vec3::new(0.0, 0.0, 9.0), Vec3::ZERO, 10.0);
        let ameacas = [ameaca(14.0, 0.0)];
        assert!(!decidir(&p, &ameacas, 1.0 / 60.0).atirar);
    }

    /// Aplica as decisões do piloto a uma câmera orbital, como o laço faz.
    fn perseguir(camera: &mut OrbitalCamera, alvo: Vec3, segundos: f32) -> f32 {
        let dt = 1.0 / 60.0;
        let mut tempo = 0.0;
        let mut erro = std::f32::consts::PI;
        while tempo < segundos {
            let panorama = panorama_da_camera(camera, REACAO + 1.0);
            let ameacas = [Ameaca {
                posicao: alvo,
                distancia_ao_centro: (alvo.x * alvo.x + alvo.y * alvo.y + alvo.z * alvo.z).sqrt(),
            }];
            let decisao = decidir(&panorama, &ameacas, dt);
            camera.yaw += decisao.yaw;
            camera.pitch = (camera.pitch + decisao.pitch).clamp(
                crate::gameplay::camera::PITCH_MINIMO,
                crate::gameplay::camera::PITCH_MAXIMO,
            );
            erro = angulo_entre(direcao_de(&panorama_da_camera(camera, 0.0)), {
                let v = Vec3::new(
                    alvo.x - panorama.olho.x,
                    alvo.y - panorama.olho.y,
                    alvo.z - panorama.olho.z,
                );
                v.normalized()
            });
            tempo += dt;
            if erro <= TOLERANCIA * 0.5 {
                break;
            }
        }
        erro
    }

    #[test]
    fn o_piloto_converge_no_alvo_longe_e_baixo() {
        // O caso real: cubo a 15 m, quase na altura do asteroide, com a câmera
        // olhando de 22° acima. A primeira versão **não** convergia — ela subia
        // até o limite de 80° olhando para o chão (medido: 0 abates em cinco
        // partidas inteiras).
        let mut camera = OrbitalCamera::new();
        let erro = perseguir(&mut camera, Vec3::new(0.0, 0.0, -15.0), 3.0);
        assert!(
            erro <= TOLERANCIA * 0.5,
            "erro angular {erro:.3} rad depois de 3 s — não convergiu"
        );
    }

    #[test]
    fn o_piloto_converge_no_alvo_alto_e_no_lateral() {
        // Um cubo no topo da faixa de spawn e outro a 90° de lado: os dois eixos
        // de giro têm de convergir, não só o `yaw`.
        let mut camera = OrbitalCamera::new();
        let alto = perseguir(&mut camera, Vec3::new(0.0, 4.0, -14.0), 3.0);
        assert!(alto <= TOLERANCIA * 0.5, "alvo alto: erro {alto:.3}");

        let mut camera = OrbitalCamera::new();
        let lateral = perseguir(&mut camera, Vec3::new(15.0, 0.0, 0.0), 4.0);
        assert!(
            lateral <= TOLERANCIA * 0.5,
            "alvo lateral: erro {lateral:.3}"
        );
    }

    #[test]
    fn a_decisao_e_deterministica() {
        // Sem isto, comparar duas dificuldades mediria o acaso.
        let p = panorama(Vec3::new(2.0, 3.0, 8.0), Vec3::ZERO, 1.0);
        let ameacas = [ameaca(3.0, 1.0), ameaca(-4.0, 2.0), ameaca(0.5, 6.0)];
        let a = decidir(&p, &ameacas, 1.0 / 60.0);
        for _ in 0..10 {
            assert_eq!(decidir(&p, &ameacas, 1.0 / 60.0), a);
        }
    }

    #[test]
    fn ameaca_invalida_e_ignorada() {
        let p = panorama(Vec3::new(0.0, 0.0, 9.0), Vec3::ZERO, 5.0);
        let invalidas = [
            Ameaca {
                posicao: Vec3::new(f32::NAN, 0.0, 0.0),
                distancia_ao_centro: 3.0,
            },
            Ameaca {
                posicao: Vec3::new(0.0, 0.0, 3.0),
                distancia_ao_centro: f32::NAN,
            },
        ];
        assert_eq!(escolher_alvo(&p, &invalidas), None);
    }

    #[test]
    fn o_panorama_vem_da_camera_orbital() {
        let camera = OrbitalCamera::new();
        let p = panorama_da_camera(&camera, 0.5);
        let direta = camera.camera(640, 480);
        // A posição calculada aqui tem de ser a mesma que a câmera usa.
        let dx = p.olho.x - direta.position.x;
        let dy = p.olho.y - direta.position.y;
        let dz = p.olho.z - direta.position.z;
        assert!(
            (dx * dx + dy * dy + dz * dz).sqrt() < 1e-4,
            "panorama e câmera discordam da posição do olho"
        );
        assert_eq!(p.tempo_no_alvo, 0.5);
    }

    #[test]
    fn tempo_invalido_nao_faz_o_piloto_girar() {
        let p = panorama(Vec3::new(0.0, 0.0, 9.0), Vec3::ZERO, 5.0);
        let ameacas = [ameaca(12.0, 0.0)];
        for dt in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            let decisao = decidir(&p, &ameacas, dt);
            assert_eq!(decisao.yaw, 0.0, "dt {dt}");
            assert_eq!(decisao.pitch, 0.0, "dt {dt}");
            assert!(!decisao.atirar);
        }
    }
}
