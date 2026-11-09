//! Pool fixo de inimigos.
//!
//! ## Por que pool e não `Vec`
//!
//! O envelope deste projeto é de **15 cubos simultâneos** (R5). Um `Vec` que
//! cresce e encolhe a cada morte aloca durante a partida, e alocação no caminho
//! quente é a classe de problema que aparece no aparelho como travada de 20 ms e
//! não aparece no PC. Um array de 15 com `alive` faz o custo ser constante e
//! previsível — e o `spawn` devolve `None` quando lota, que é uma resposta, não
//! um `panic`.
//!
//! ## O índice é estável
//!
//! `spawn` devolve o índice do slot e `despawn` recebe o mesmo índice. Reusar
//! slot é o que mantém o consumo de memória plano; o preço é que quem guardou um
//! índice precisa checar `alive` depois, porque o slot pode ter sido reocupado
//! por outro cubo.

use crate::gameplay::enemy::Enemy;
use giochi_engine_3d::math3d::Vec3;

/// Quantos cubos podem existir ao mesmo tempo.
pub const CAPACIDADE: usize = 15;

/// Array fixo de inimigos reutilizáveis.
#[derive(Debug, Clone, PartialEq)]
pub struct EnemyPool {
    inimigos: [Enemy; CAPACIDADE],
    ativos: usize,
}

impl EnemyPool {
    /// Pool vazio.
    pub fn new() -> Self {
        Self {
            inimigos: [Enemy::morto(); CAPACIDADE],
            ativos: 0,
        }
    }

    /// Ocupa o primeiro slot livre.
    ///
    /// `None` quando os 15 já estão no ar — quem chama decide se espera o próximo
    /// quadro ou se perde a onda.
    pub fn spawn(&mut self, posicao: Vec3, velocidade: f32) -> Option<usize> {
        for (indice, slot) in self.inimigos.iter_mut().enumerate() {
            if !slot.alive {
                *slot = Enemy::spawn_at(posicao, velocidade);
                self.ativos += 1;
                return Some(indice);
            }
        }
        None
    }

    /// Mata o inimigo do slot. Devolve `false` se ele já estava morto.
    ///
    /// Não decrementa duas vezes: um `despawn` repetido é o defeito que faria
    /// `ativos` divergir do número de cubos vivos, e aí o pool "lota" com slots
    /// livres.
    pub fn despawn(&mut self, indice: usize) -> bool {
        match self.inimigos.get_mut(indice) {
            Some(slot) if slot.alive => {
                slot.alive = false;
                self.ativos -= 1;
                true
            }
            _ => false,
        }
    }

    /// Quantos estão vivos.
    pub fn active_count(&self) -> usize {
        self.ativos
    }

    /// Há espaço para mais um?
    pub fn tem_espaco(&self) -> bool {
        self.ativos < CAPACIDADE
    }

    /// Iterador sobre os vivos, com o índice do slot.
    pub fn iter_alive(&self) -> impl Iterator<Item = (usize, &Enemy)> {
        self.inimigos
            .iter()
            .enumerate()
            .filter(|(_, inimigo)| inimigo.alive)
    }

    /// Iterador mutável sobre os vivos.
    pub fn iter_alive_mut(&mut self) -> impl Iterator<Item = (usize, &mut Enemy)> {
        self.inimigos
            .iter_mut()
            .enumerate()
            .filter(|(_, inimigo)| inimigo.alive)
    }

    /// Lê o slot, vivo ou não.
    pub fn get(&self, indice: usize) -> Option<&Enemy> {
        self.inimigos.get(indice)
    }

    /// Esvazia o pool (reinício de partida).
    pub fn limpar(&mut self) {
        self.inimigos = [Enemy::morto(); CAPACIDADE];
        self.ativos = 0;
    }
}

impl Default for EnemyPool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enche(pool: &mut EnemyPool) -> usize {
        let mut criados = 0;
        while pool.spawn(Vec3::new(10.0, 0.0, 0.0), 3.0).is_some() {
            criados += 1;
        }
        criados
    }

    #[test]
    fn o_pool_comeca_vazio() {
        let pool = EnemyPool::new();
        assert_eq!(pool.active_count(), 0);
        assert_eq!(pool.iter_alive().count(), 0);
        assert!(pool.tem_espaco());
    }

    #[test]
    fn lota_exatamente_na_capacidade() {
        let mut pool = EnemyPool::new();
        assert_eq!(enche(&mut pool), CAPACIDADE);
        assert_eq!(pool.active_count(), CAPACIDADE);
        assert!(!pool.tem_espaco(), "15 vivos e ainda diz que tem espaço");
        assert_eq!(pool.iter_alive().count(), CAPACIDADE);
    }

    #[test]
    fn despawn_libera_o_slot_e_o_proximo_reusa() {
        let mut pool = EnemyPool::new();
        let primeiro = pool.spawn(Vec3::new(10.0, 0.0, 0.0), 1.0).expect("slot");
        assert!(pool.despawn(primeiro));
        assert_eq!(pool.active_count(), 0);
        let de_novo = pool.spawn(Vec3::new(5.0, 0.0, 0.0), 1.0).expect("slot");
        assert_eq!(de_novo, primeiro, "o slot livre devia ser reusado");
    }

    #[test]
    fn despawn_duas_vezes_nao_desconta_duas() {
        // Sem a guarda, `ativos` divergiria do número de vivos e o pool passaria
        // a dizer que está cheio com slots livres.
        let mut pool = EnemyPool::new();
        let indice = pool.spawn(Vec3::ZERO, 1.0).expect("slot");
        assert!(pool.despawn(indice));
        assert!(
            !pool.despawn(indice),
            "o segundo despawn devia ser recusado"
        );
        assert_eq!(pool.active_count(), 0);
    }

    #[test]
    fn despawn_fora_da_faixa_e_recusado_sem_panico() {
        let mut pool = EnemyPool::new();
        assert!(!pool.despawn(0));
        assert!(!pool.despawn(CAPACIDADE));
        assert!(!pool.despawn(usize::MAX));
    }

    #[test]
    fn o_iterador_so_devolve_vivos() {
        let mut pool = EnemyPool::new();
        for _ in 0..5 {
            pool.spawn(Vec3::new(10.0, 0.0, 0.0), 1.0).expect("slot");
        }
        pool.despawn(0);
        pool.despawn(2);
        let indices: Vec<usize> = pool.iter_alive().map(|(i, _)| i).collect();
        assert_eq!(indices, vec![1, 3, 4]);
        assert_eq!(pool.active_count(), 3);
    }

    #[test]
    fn limpar_devolve_o_pool_ao_inicio() {
        let mut pool = EnemyPool::new();
        enche(&mut pool);
        pool.limpar();
        assert_eq!(pool, EnemyPool::new());
    }

    #[test]
    fn os_vivos_se_movem_pelo_iterador_mutavel() {
        let mut pool = EnemyPool::new();
        pool.spawn(Vec3::new(10.0, 0.0, 0.0), 5.0).expect("slot");
        for (_, inimigo) in pool.iter_alive_mut() {
            inimigo.update(1.0, Vec3::ZERO);
        }
        let (_, primeiro) = pool.iter_alive().next().expect("um vivo");
        assert!((primeiro.position.x - 5.0).abs() < 1e-4);
    }
}
