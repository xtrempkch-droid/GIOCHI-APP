#!/usr/bin/env python3
"""Executa comandos na console serial de um aparelho Giochi.

    console-serial.py <porta> <baud> "<comando>" ["<comando>" ...]

Imprime a saída de cada comando, entre marcadores. Código de saída 0 se todos
rodaram (o status vem do próprio aparelho), 1 se algum falhou, 2 se a porta não
respondeu.

# Por que esta ferramenta existe

O **R35S não tem rádio** e o `dwc2` nasce como *host*, então não há SSH nem
`usb0`: a console serial é o único canal. `aparelho.sh`, que é todo feito de
`ssh`/`scp`, não alcança esse aparelho — e por isso o toolkit ficava sem como
verificar nada nele.

# Por que marcadores, e não `sleep N e lê o que vier`

Porque o aparelho **ecoa o que recebe**. Sem marcador, a saída do comando se
mistura com o eco do próprio comando, e "achou" e "não achou" viram a mesma
leitura. Aqui o comando é enviado entre dois `echo` com um número de série
aleatório, e a leitura termina no marcador final — não num tempo.

# Por que o `$?` viaja no marcador

Porque o código de saída **é** o dado: `capacidades` que devolve texto vazio com
status 0 e com status 1 significam coisas diferentes. Colar o status no marcador
final evita um comando extra só para perguntar, que teria o mesmo problema.

# O detalhe que quebra tudo

O adaptador CH341 é **acesso exclusivo** e cai do barramento de vez em quando.
Sem `--privileged` o container não abre a porta; sem `-v /dev:/dev` o container
morre quando o nó some. Ver `scripts/console-r35s.sh`.
"""

import secrets
import sys
import time

try:
    import serial
except ImportError:
    sys.stderr.write("pyserial ausente: rode dentro da imagem (ver console-r35s.sh)\n")
    sys.exit(2)


def abrir(porta: str, baud: int) -> "serial.Serial":
    # `timeout` curto: a leitura é por marcador, e um timeout longo faria uma
    # falha de contato parecer "o comando está demorando".
    return serial.Serial(porta, baud, timeout=0.2, write_timeout=5)


def limpar(ser: "serial.Serial") -> None:
    """Descarta o que estiver no buffer antes de começar.

    O prompt do shell e sobras de um comando anterior chegam misturados; sem
    limpar, a primeira saída capturada seria lixo antigo.
    """
    ser.reset_input_buffer()


def executar(ser: "serial.Serial", comando: str, espera: float = 25.0) -> tuple[str, int | None]:
    """Envia um comando e devolve (saída, status).

    O status é `None` quando o marcador não voltou — que é informação: significa
    que o comando derrubou a console (panic, reboot, ou o aparelho caiu).
    """
    etiqueta = secrets.token_hex(4)
    # `printf` e não `echo`: o `echo` do busybox interpreta `\\` e pode comer
    # sequências que o comando mandou.
    # O marcador é montado **no aparelho**, com `%s`, e não escrito literal na
    # linha de comando. Motivo: o shell ecoa a linha, e um marcador literal
    # apareceria no eco antes da hora — a leitura pararia no eco e a resposta
    # viria vazia. Custou duas tentativas até ficar claro.
    envio = (
        "printf '\\n@@%s-%s@@\\n' INI {etiqueta}; "
        "{comando}; "
        "printf '@@%s-%s:%d@@\\n' FIM {etiqueta} $?\n"
    ).format(etiqueta=etiqueta, comando=comando)
    ser.write(envio.encode("utf-8", "replace"))
    ser.flush()

    limite = time.time() + espera
    inicio_b = f"@@INI-{etiqueta}@@".encode()
    fim_b = f"@@FIM-{etiqueta}:".encode()

    # Duas fases, e a ordem importa. O shell **ecoa** o comando, e o eco chega
    # antes do marcador: acumular tudo e recortar depois deixava pedaços do eco
    # nas respostas de comandos longos, que o terminal quebra em várias linhas.
    # Descartar até o marcador de início resolve na origem.
    acumulado = b""
    while time.time() < limite and inicio_b not in acumulado:
        pedaco = ser.read(4096)
        if pedaco:
            acumulado += pedaco
        else:
            time.sleep(0.02)

    if inicio_b not in acumulado:
        return acumulado.decode("utf-8", "replace").strip(), None

    acumulado = acumulado.split(inicio_b, 1)[1]
    while time.time() < limite and fim_b not in acumulado:
        pedaco = ser.read(4096)
        if pedaco:
            acumulado += pedaco
        else:
            time.sleep(0.02)

    texto = acumulado.decode("utf-8", "replace")
    if fim_b.decode() not in texto:
        return texto.strip(), None

    etiqueta_fim = f"@@FIM-{etiqueta}"
    marca = texto.find(etiqueta_fim)
    corpo = texto[:marca]
    try:
        status = int(texto[marca + len(f"@@FIM-{etiqueta}:") :].split("@@", 1)[0])
    except (ValueError, IndexError):
        status = None

    # O eco do comando volta junto. Removo as linhas que são exatamente o que
    # enviei, para a saída ser lida por humano e por `grep`.
    enviadas = {l.strip() for l in envio.strip().splitlines() if l.strip()}
    linhas = [l for l in corpo.replace("\r", "").splitlines() if l.strip() not in enviadas]
    return "\n".join(linhas).strip(), status


def principal(argv: list[str]) -> int:
    if len(argv) < 4:
        sys.stderr.write(__doc__.split("\n")[1].strip() + "\n")
        return 2

    porta, baud = argv[1], int(argv[2])
    comandos = argv[3:]

    try:
        ser = abrir(porta, baud)
    except Exception as exc:  # noqa: BLE001 - a mensagem do driver é o diagnóstico
        sys.stderr.write(f"ERRO não consegui abrir {porta}: {exc}\n")
        return 2

    with ser:
        limpar(ser)
        # Um Enter solto confirma que o shell está vivo antes de mandar trabalho.
        ser.write(b"\n")
        ser.flush()
        time.sleep(0.4)
        ser.reset_input_buffer()

        falhou = False
        for comando in comandos:
            print(f"$ {comando}")
            saida, status = executar(ser, comando)
            if saida:
                print(saida)
            if status is None:
                print("!! sem resposta — a console não devolveu o marcador")
                falhou = True
            elif status != 0:
                print(f"!! status {status}")
                falhou = True
            print()

        return 1 if falhou else 0


if __name__ == "__main__":
    sys.exit(principal(sys.argv))
