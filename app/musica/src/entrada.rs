//! Entrada: controle embutido (evdev) e teclado.
//!
//! ## Por que evdev, e não o gamepad do SDL
//!
//! No aparelho, os botões vêm de `gpio-keys`/`rocknix-joypad` e o
//! `SDL_GameControllerDB` não tem mapeamento para eles: a API de gamepad do SDL
//! devolve `NULL` e o app abriria, desenharia e **nenhum botão responderia**,
//! sem erro nenhum no log. O launcher de produção já lê `/dev/input` direto por
//! esse motivo, e o leitor é o **mesmo arquivo** (`#[path]`), não uma cópia.
//!
//! ## Por que o `Mapper` do launcher
//!
//! Borda de botão e repetição do direcional são comportamento medido no
//! aparelho: segurar o direcional anda depois de ~230 ms e repete a cada
//! ~100 ms. Uma segunda implementação disso teria outra curva, e o usuário
//! sentiria a diferença entre o launcher e o app de música.

use std::path::Path;

use giochi_launcher_core::gamepad::Mapper;
use giochi_launcher_core::{Cmd, Dir};
use giochi_musica_core::Acao;

#[path = "../../launcher/crates/ui-sdl/src/input_evdev.rs"]
mod evdev;

use evdev::{ButtonsMap, EvdevInput};

/// Mapa de botões do sistema. O mesmo caminho do launcher: o mapa é **do
/// aparelho**, não do app, e um segundo arquivo divergiria do primeiro.
pub const MAPA_PADRAO: &str = evdev::DEFAULT_MAP_PATH;

/// Leitura de entrada do app.
pub struct Entrada {
    evdev: Option<EvdevInput>,
    mapper: Mapper,
    /// Evita repetir "sair" enquanto o START está fisicamente pressionado.
    saindo: bool,
}

impl Entrada {
    pub fn abrir() -> Entrada {
        let mapa = ButtonsMap::load(Path::new(MAPA_PADRAO));
        if !mapa.is_complete() {
            eprintln!(
                "entrada: aviso — o mapa de botões não está completo ({} símbolos)",
                MAPA_PADRAO
            );
        }
        let diretorio = Path::new("/dev/input");
        let evdev = if diretorio.is_dir() {
            let leitor = EvdevInput::open(diretorio, mapa);
            eprintln!(
                "entrada: {} dispositivo(s) em /dev/input",
                leitor.device_count()
            );
            Some(leitor)
        } else {
            eprintln!("entrada: sem /dev/input — só teclado (é o caso do PC)");
            None
        };
        Entrada {
            evdev,
            mapper: Mapper::new(),
            saindo: false,
        }
    }

    /// Lê um quadro de entrada e devolve as ações, na ordem em que chegaram.
    ///
    /// `teclado` vem do SDL; o controle, do evdev. Os dois podem coexistir (um
    /// aparelho com teclado USB), e a ordem preservada é a de leitura.
    pub fn acoes(&mut self, teclado: Vec<AcaoDoTeclado>) -> Vec<Acao> {
        let mut acoes = Vec::new();

        for tecla in teclado {
            match tecla {
                AcaoDoTeclado::Acao(acao) => acoes.push(acao),
                AcaoDoTeclado::Sair => {
                    if !self.saindo {
                        self.saindo = true;
                        acoes.push(Acao::Sair);
                    }
                }
            }
        }

        let Some(leitor) = self.evdev.as_mut() else {
            return acoes;
        };
        leitor.poll();
        for cmd in self.mapper.update(leitor.frame()) {
            if let Some(acao) = de_cmd(&cmd) {
                acoes.push(acao);
            }
        }

        // As teclas de volume do aparelho existem fora do mapa de botões (são
        // `KEY_VOLUMEUP`/`KEY_VOLUMEDOWN`, do driver do PMIC). O launcher as
        // trata como volume do mixer; aqui idem.
        let (menos, mais) = leitor.volume_pressed();
        if menos {
            acoes.push(Acao::Volume(-1));
        }
        if mais {
            acoes.push(Acao::Volume(1));
        }

        if leitor.start_pressed() && !self.saindo {
            self.saindo = true;
            acoes.push(Acao::Sair);
        } else if !leitor.start_pressed() {
            self.saindo = false;
        }

        acoes
    }
}

/// Teclado do SDL, já traduzido para ação (ou pedido de saída).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcaoDoTeclado {
    Acao(Acao),
    Sair,
}

/// Traduz um comando do launcher na ação equivalente do tocador.
///
/// A tabela é explícita porque os dois apps compartilham os **botões**, não os
/// significados: no launcher `A` abre a ficha do jogo, aqui toca a música.
pub fn de_cmd(cmd: &Cmd) -> Option<Acao> {
    match cmd {
        Cmd::Nav(Dir::Left) => Some(Acao::Mover(-1)),
        Cmd::Nav(Dir::Right) => Some(Acao::Mover(1)),
        Cmd::Nav(Dir::Up) => Some(Acao::Volume(1)),
        Cmd::Nav(Dir::Down) => Some(Acao::Volume(-1)),
        Cmd::Confirm => Some(Acao::Alternar),
        Cmd::Options => Some(Acao::Proxima),
        Cmd::Search => Some(Acao::Anterior),
        Cmd::Back => Some(Acao::Parar),
        Cmd::TabPrev => Some(Acao::Volume(-1)),
        Cmd::TabNext => Some(Acao::Volume(1)),
        // Select = reler a pasta. Não é um comando do launcher, mas é o botão
        // que sobra no controle e o único jeito de descobrir música copiada
        // depois de o app abrir.
        Cmd::ToggleQuickSettings => Some(Acao::Recarregar),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direcional_move_um_passo_por_borda() {
        assert_eq!(de_cmd(&Cmd::Nav(Dir::Left)), Some(Acao::Mover(-1)));
        assert_eq!(de_cmd(&Cmd::Nav(Dir::Right)), Some(Acao::Mover(1)));
    }

    #[test]
    fn botoes_de_face_tem_o_significado_do_rodape() {
        assert_eq!(de_cmd(&Cmd::Confirm), Some(Acao::Alternar));
        assert_eq!(de_cmd(&Cmd::Options), Some(Acao::Proxima));
        assert_eq!(de_cmd(&Cmd::Search), Some(Acao::Anterior));
        assert_eq!(de_cmd(&Cmd::Back), Some(Acao::Parar));
    }

    #[test]
    fn ombros_e_direcional_vertical_mexem_no_volume() {
        assert_eq!(de_cmd(&Cmd::TabNext), Some(Acao::Volume(1)));
        assert_eq!(de_cmd(&Cmd::TabPrev), Some(Acao::Volume(-1)));
        assert_eq!(de_cmd(&Cmd::Nav(Dir::Up)), Some(Acao::Volume(1)));
        assert_eq!(de_cmd(&Cmd::Nav(Dir::Down)), Some(Acao::Volume(-1)));
    }

    #[test]
    fn comandos_do_launcher_sem_equivalente_nao_viram_acao() {
        // "Nada" é melhor que um mapeamento inventado: um comando que o app não
        // entende não pode mover o foco nem trocar a música.
        assert_eq!(de_cmd(&Cmd::ToggleHud), None);
        assert_eq!(de_cmd(&Cmd::PowerShort), None);
        assert_eq!(de_cmd(&Cmd::HideToast), None);
        assert_eq!(de_cmd(&Cmd::SubNext), None);
    }

    #[test]
    fn select_rele_a_pasta() {
        assert_eq!(de_cmd(&Cmd::ToggleQuickSettings), Some(Acao::Recarregar));
    }

    #[test]
    fn todo_botao_do_rodape_tem_mapeamento() {
        // A lista do rodapé é a promessa da tela; se ela citar uma tecla, o
        // `de_cmd` tem de conhecer o comando correspondente.
        let prometidas = [
            Cmd::Confirm,
            Cmd::Options,
            Cmd::Search,
            Cmd::TabPrev,
            Cmd::TabNext,
        ];
        for cmd in prometidas {
            assert!(de_cmd(&cmd).is_some(), "{cmd:?} prometido e não mapeado");
        }
    }
}
