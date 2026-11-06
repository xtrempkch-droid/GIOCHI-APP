# Wi-Fi e Bluetooth

Como a aba de configurações deixa de ser decorativa e passa a conectar de
verdade — e o que foi medido no aparelho para provar isso.

---

## 1. O problema, nas palavras do usuário

> "na haba config nao e navegavel"
> "as configuraçoes do wifi e bluetooh nao tem lugar para parear e por senha"

Duas queixas distintas, e nenhuma delas era "falta de código": eram **duas
ausências de contrato** entre o desenho e o núcleo.

| Queixa | Causa real |
| --- | --- |
| a aba não é navegável | existia `Zone::SettingsSidebar` e **não** existia `Zone::SettingsContent` |
| não há onde parear/digitar senha | o painel da direita era texto puro: nenhuma linha tinha `hits`, nenhum overlay existia |

O painel da direita desenhava `("Wi-Fi", "Ligado")` e `("Bluetooth", "Ativo")`
de um campo booleano que **ninguém alternava**. Era informação estável e falsa:
sempre "Ligado", independente de haver conexão. Não havia `enter`
correspondente porque não havia nada atrás dele.

A lição, que já apareceu antes neste projeto na forma do `year_label()`: **texto
desenhado não é funcionalidade.** Um retângulo com "Wi-Fi — Ligado" parece um
controle e não é; o usuário descobre isso apertando A e nada acontecendo.

---

## 2. O desenho

```text
Configurações
├── coluna esquerda  Zone::SettingsSidebar   (6 categorias)
└── painel direito   Zone::SettingsContent   (linhas da categoria)
        └── "Rede e BT" linha 0 → Overlay::WifiList        Zone::WifiNetworks
        └── "Rede e BT" linha 1 → Overlay::BtList           Zone::BtDevices
                                    ↓ escolher rede protegida
                              Overlay::WifiPassword         Zone::PasswordKeys
```

Navegação: `→` da coluna entra no painel, `←` do painel volta para a coluna na
categoria em que estava. Andar na coluna **muda a categoria na hora** — exigir
`A` em cada item fazia o painel mostrar o conteúdo anterior enquanto o destaque
já estava em outro.

Categoria sem linhas (`sobre`) **não recebe `→`**: entrar num painel vazio
prenderia o foco sem nada para onde ir.

### A senha nunca é o que se vê

O campo mostra `•` repetido, com o comprimento sempre visível — sem isso não se
sabe se a tecla pegou (`X` alterna para texto claro). A senha é limpa ao abrir e
ao confirmar; não vai para disco nem para o estado depois da conexão
(`SemFio::limpar_senha`).

---

## 3. Quem executa

O núcleo é puro e continua testável sem hardware. Ele **marca** o que o usuário
quer; a camada `ui-sdl` (`semfio.rs`) executa.

```text
estado diz "escaneando"  →  SemFio::pedir(EscanearWifi)  →  canal  →  wpa_cli
                                                              ↓
                          dispatch(Cmd::RedesWifi)  ←  canal  ←  thread
```

Três regras de `App`:

1. **`sync_sem_fio` reage a transições**, não a estados. `escaneando` fica
   verdadeiro por vários segundos; sem comparar antes/depois, a varredura seria
   pedida a cada quadro.
2. **`App::new` concilia o estado inicial.** `--rede-real` marca `escaneando`
   antes da janela abrir, e o retorno de um jogo reabre o `App` com o estado que
   estava na tela. Como não há transição, o pedido sai na construção — sem isso a
   tela fica "procurando…" para sempre.
3. **Um comando com efeito só no estado seria uma segunda rota.** Só
   `ConectarSenha` é pass-through de verdade (depende da senha digitada); os
   outros três efeitos são visíveis no estado.

---

## 4. O que foi medido no X55 (06/11/2026)

### Ambiente

| Item | Valor |
| --- | --- |
| `wpa_supplicant` | 2.10, rodando (pid 194) no boot, socket em `/var/run/wpa_supplicant/wlan0` |
| `wpa_cli` | `/usr/sbin/wpa_cli` |
| `bluetoothctl` | 5.72, controlador `A8:4F:A4:C9:98:4C`, `Powered: yes`, `Pairable: no` |
| `ip` | `/sbin/ip` |
| `wlan0` no boot | **DOWN** — o `S40wifi` não levanta quando o `wpa_supplicant.conf` está vazio |
| PATH do `giochi-init` | `/bin:/sbin:/usr/bin:/usr/sbin` |

**Não existe `timeout` na imagem.** As janelas usam o `--timeout` do próprio
`bluetoothctl` e uma espera fixa no `wpa_cli`. Um comando que não existe falha em
silêncio se ninguém olhar — e essa foi a surpresa do dia.

`/etc` é **gravável** (`/dev/root on / type ext4 (rw,relatime)`), então
`save_config` funciona.

### Resultados

```text
wi-fi: interface wlan0 levantada
wi-fi: 11 rede(s) em wlan0
bluetooth: 1 aparelho(s) conhecido(s)
```

O launcher sobe a interface sozinho, varre, e a lista **real** aparece na tela:
painel 620×606 px no espaço de projeto 1280×720, 11 redes + a linha "Procurar de
novo", 275 cores, 0,00 % de preto.

### A senha errada prova a corrente inteira

Teste com `--conectar "ORLEANS:senha-errada-de-proposito"`:

```text
--conectar: ORLEANS (senha de 25 caractere(s))
wi-fi: interface wlan0 levantada
rede: a rede não respondeu (senha errada ou fora de alcance)
```

E no aparelho, depois:

```text
network id / ssid / bssid / flags
0		any	[DISABLED]
1	ORLEANS	any	[TEMP-DISABLED]
```

Duas conclusões medidas, e nenhuma delas é "funcionou":

1. **A corrente de comandos inteira executou** — `add_network` criou a rede 1 com
   o SSID certo; `set_network`, `enable_network` e `select_network` responderam
   OK. Se qualquer um tivesse falhado, a mensagem seria `set_network: FAIL`, não
   um estado do supplicant.
2. **`save_config` não rodou** — `/etc/wpa_supplicant.conf` ficou intacto. É a
   guarda funcionando: uma senha errada não é persistida, então o aparelho não
   tenta sozinho no próximo boot e falha sem ninguém entender por quê.

**Esperar a associação é o que separa "os comandos aceitaram" de "conectou".**
Sem essa espera, `select_network` respondendo OK devolveria sucesso e a tela diria
"Conectado" para uma rede que nunca conectou.

### Bluetooth

`bluetoothctl --timeout 8 scan on` é a janela de descoberta; o resultado aparece
com `bluetoothctl devices`. `Pairable: no` é o padrão do controlador e o pedido de
pareamento é recusado pelo próprio aparelho, então a varredura liga `pairable on`
e registra o agente `KeyboardDisplay` (teclado pede código).

O caminho de **erro** foi exercitado com um MAC inexistente:

```text
--parear: 00:00:00:00:00:00
rede: Device 00:00:00:00:00:00 not available
```

A mensagem real do `bluetoothctl` chega à tela. `AuthenticationFailed`,
`already exists` e `not available` pedem ações diferentes do usuário, e um
"falhou" genérico esconderia qual é.

---

## 5. O que NÃO foi provado

Honestidade, como no resto do arquivo:

| Alegação | Estado |
| --- | --- |
| conectar a uma rede **real** com a senha certa | **não testado** — não tenho a senha de nenhuma rede do usuário |
| parear um aparelho BT real | **não testado** — nenhum teclado/headset disponível, e parear com o que apareceu na varredura (um aparelho de terceiro) seria abusivo |
| **X35H** | **não testado** — as telas de rede estreiam nesta rodada e o X35H roda 640×480 (`rotate=0`), o único caminho de desenho que o X55 não exercita |
| FPS com a thread de rede viva | não medido |

O X35H é o risco concreto: em 640×480 o painel de lista tem `m.e(620)` reduzido e
11 linhas podem não caber. `node scripts/verificar-telas.mjs` mede 640×480 no PC
(23 % de ocupação, todas as telas cabem), mas **medida no PC não é medida no
aparelho** — é a mesma distinção que fez o letterbox passar despercebido antes.

---

## 6. Diagnóstico em campo

```bash
# no PC, com dados de exemplo — serve para olhar o desenho
SDL_VIDEODRIVER=dummy app/launcher/bin/giochi-launcher \
  --tamanho 640x480 --tela wifi --dump-frame /tmp/wifi.bmp

# no aparelho, com a varredura de verdade
giochi-launcher --tela wifi --rede-real --esperar-ms 12000 --dump-frame /tmp/wifi.bmp

# exercita a conexão sem dedo na tela
giochi-launcher --conectar "SSID:senha"
giochi-launcher --parear AA:BB:CC:DD:EE:FF
```

`--esperar-ms` existe porque o `--dump-frame` grava no **primeiro** quadro: sem a
espera, a foto da lista sai em "Procurando…" e não prova nada sobre a lista.

`--foco zona:indice` existe porque um quadro parado no estado inicial nunca mostra
o anel de foco — e foi justamente a falta de foco no painel a queixa do usuário.
Sem ele, "tem anel" e "não tem anel" produzem o mesmo BMP.

`--rede-real` existe porque as telas abrem com dados de exemplo, o que é bom para
fotografar o layout e péssimo para provar que o `wpa_cli` funciona: a foto
mostraria números inventados e chamaria de sucesso.

---

## 7. Como o foco e o desenho não divergem

`SETTINGS_CATEGORIES` no núcleo decide quantas linhas o foco pode percorrer
(`settings_rows`). O desenho monta as mesmas linhas em
`views::linhas_da_categoria`. Dois lugares descrevendo a mesma lista foi
exatamente como o carrossel ficou dessincronizado do foco.

Por isso há um teste em `views::tests`:

```rust
foco_e_desenho_concordam_no_numero_de_linhas
```

Ele percorre as seis categorias e reprova se os dois números divergirem. É o elo
que faltava — sem ele a divergência não dá erro em lugar nenhum: o foco
simplesmente para antes da última linha, ou anda por uma linha que não existe.
