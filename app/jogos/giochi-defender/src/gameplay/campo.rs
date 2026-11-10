//! O campo de batalha: onde os cubos nascem, derivado da câmera.
//!
//! ## O problema que este módulo resolve
//!
//! O bloco 1b fixou a casca de spawn em `15..20` unidades com a câmera a 9. Isso
//! funcionava porque `15` é confortavelmente maior que `9`: tudo nascia **na
//! frente** do jogador.
//!
//! No modo fácil a câmera vai para **18** (o dobro, pedido do dono) — e a casca
//! de `15..20` passaria a envolver o olho. Metade dos cubos nasceria atrás da
//! câmera, e o sintoma seria "os cubos aparecem do nada no meio da tela", que
//! manda procurar defeito no spawn em vez da câmera.
//!
//! A correção é derivar a casca da distância: [`Campo::para_distancia`] garante
//! uma folga mínima **além do olho** e preserva exatamente os números do 1b
//! quando a distância é a de referência.
//!
//! ## E a velocidade tem de acompanhar
//!
//! Se a casca cresce e a velocidade não, o cubo demora mais para chegar e o modo
//! fácil vira "modo lento": o jogador vê a ameaça por oito segundos e não sente
//! pressão nenhuma. [`Campo::fator_velocidade`] mantém o **tempo de viagem**, e
//! assim o que muda entre os modos é o que o dono pediu — o quanto se vê — e não
//! a duração da partida.

use crate::gameplay::partida::{ALTURA_SPAWN, RAIO_SPAWN_MAX, RAIO_SPAWN_MIN};

/// Folga mínima entre o olho da câmera e a casca de spawn.
///
/// É o que garante que todo cubo nasce à frente do jogador. 6,0 é o valor que
/// reproduz o `RAIO_SPAWN_MIN = 15` do 1b a partir da câmera a 9.
pub const FOLGA_ALEM_DA_CAMERA: f32 = 6.0;

/// Profundidade da casca, em unidades. Igual ao `20 - 15` do bloco 1b.
pub const PROFUNDIDADE_DA_CASCA: f32 = RAIO_SPAWN_MAX - RAIO_SPAWN_MIN;

/// Onde os cubos nascem, para uma distância de câmera qualquer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Campo {
    /// Raio mínimo de spawn.
    pub raio_min: f32,
    /// Raio máximo de spawn.
    pub raio_max: f32,
    /// Meia-altura da faixa em torno do horizonte.
    pub altura: f32,
}

impl Campo {
    /// Campo que corresponde à câmera a `distancia` unidades do alvo.
    ///
    /// A faixa vertical cresce na mesma proporção do raio: a `ALTURA_SPAWN` de 4
    /// foi escolhida para um raio de 15, e mantê-la fixa com a câmera ao dobro da
    /// distância faria os cubos nascerem num plano fino que a câmera vê de
    /// raspar.
    pub fn para_distancia(distancia: f32) -> Self {
        let distancia = if distancia.is_finite() && distancia > 0.0 {
            distancia
        } else {
            crate::gameplay::modo::DISTANCIA_DE_REFERENCIA
        };
        let raio_min = (distancia + FOLGA_ALEM_DA_CAMERA).max(RAIO_SPAWN_MIN);
        let raio_max = raio_min + PROFUNDIDADE_DA_CASCA;
        let altura = ALTURA_SPAWN * raio_min / RAIO_SPAWN_MIN;
        Self {
            raio_min,
            raio_max,
            altura,
        }
    }

    /// O campo do bloco 1b, que é o de referência.
    pub fn padrao() -> Self {
        Self::para_distancia(crate::gameplay::modo::DISTANCIA_DE_REFERENCIA)
    }

    /// Quanto a velocidade tem de ser multiplicada para o **tempo de viagem**
    /// ficar igual ao da referência.
    ///
    /// Um cubo da casca até o asteroide percorre `raio_min - raio_de_colisao`.
    /// Se o raio cresce 60 %, a velocidade cresce 60 % e a viagem leva o mesmo
    /// tempo — o que muda é só o tamanho aparente na tela.
    pub fn fator_velocidade(&self) -> f32 {
        let trajeto = (self.raio_min - crate::gameplay::partida::RAIO_COLISAO_ASTEROIDE).max(1e-3);
        let referencia =
            (RAIO_SPAWN_MIN - crate::gameplay::partida::RAIO_COLISAO_ASTEROIDE).max(1e-3);
        trajeto / referencia
    }
}

impl Default for Campo {
    fn default() -> Self {
        Self::padrao()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::modo::Modo;

    #[test]
    fn a_referencia_reproduz_os_numeros_do_bloco_1b() {
        // A regressão que este teste trava: o campo padrão tem de ser **idêntico**
        // ao que o 1b mediu, senão mexer nos modos muda a partida de quem não
        // mexeu em modo nenhum.
        let campo = Campo::padrao();
        assert_eq!(campo.raio_min, RAIO_SPAWN_MIN);
        assert_eq!(campo.raio_max, RAIO_SPAWN_MAX);
        assert_eq!(campo.altura, ALTURA_SPAWN);
        assert!((campo.fator_velocidade() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn todo_cubo_nasce_a_frente_da_camera() {
        // O defeito que este teste impede: no modo fácil a câmera vai para 18 e a
        // casca de 15..20 passaria a envolvê-la — metade dos cubos nasceria
        // atrás do jogador.
        for modo in Modo::todos() {
            let distancia = modo.distancia_da_camera();
            let campo = Campo::para_distancia(distancia);
            assert!(
                campo.raio_min > distancia + 5.0,
                "{}: casca em {:.2} com câmera a {:.2} — sem folga",
                modo.nome(),
                campo.raio_min,
                distancia
            );
        }
    }

    #[test]
    fn o_campo_cresce_com_a_camera() {
        let referencia = Campo::para_distancia(9.0);
        let medio = Campo::para_distancia(Modo::Medio.distancia_da_camera());
        let facil = Campo::para_distancia(Modo::Facil.distancia_da_camera());

        assert!(medio.raio_min > referencia.raio_min);
        assert!(facil.raio_min > medio.raio_min);
        // E a faixa vertical acompanha, senão a câmera vê o plano de spawn de
        // raspão e os cubos entram pela borda da tela.
        assert!(facil.altura > medio.altura);
        assert!(medio.altura > referencia.altura);
    }

    #[test]
    fn a_velocidade_acompanha_o_campo_para_o_tempo_de_viagem_nao_mudar() {
        // Se a casca cresce e a velocidade não, o modo fácil vira "modo lento".
        for modo in Modo::todos() {
            let campo = Campo::para_distancia(modo.distancia_da_camera());
            let trajeto = campo.raio_min - crate::gameplay::partida::RAIO_COLISAO_ASTEROIDE;
            let velocidade = 3.0 * campo.fator_velocidade();
            let tempo = trajeto / velocidade;

            let referencia =
                (RAIO_SPAWN_MIN - crate::gameplay::partida::RAIO_COLISAO_ASTEROIDE) / 3.0;
            assert!(
                (tempo - referencia).abs() < 1e-3,
                "{}: viagem de {tempo:.2}s contra {referencia:.2}s",
                modo.nome()
            );
        }
    }

    #[test]
    fn distancia_invalida_cai_na_referencia() {
        // `0`, `NaN` e negativo vêm de configuração errada; devolver um campo
        // degenerado poria os cubos dentro do asteroide.
        for valor in [0.0, -5.0, f32::NAN, f32::INFINITY] {
            assert_eq!(
                Campo::para_distancia(valor),
                Campo::padrao(),
                "distância {valor}"
            );
        }
    }
}
