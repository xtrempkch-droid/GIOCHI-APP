"""Giochi Painel — interface web local das ferramentas de PC.

Existe porque a loja e o forge são CLIs, e testar o caminho completo (classificar
→ empacotar → instalar) exigia decorar três comandos e quatro caminhos. O painel
expõe as mesmas funções num navegador, sem duplicar regra nenhuma: ele **chama**
`giochi_forge` e `giochi_store`, não reimplementa.

Decisões de escopo, todas deliberadas:

* Só escuta em `127.0.0.1`. Não há autenticação porque não há rede — expor isto
  em `0.0.0.0` seria um servidor que empacota e assina pacotes para quem chegar.
* Sem framework e sem dependência: `http.server` da biblioteca padrão. É o mesmo
  motivo pelo qual os dois apps não têm dependência externa.
* Não assina sozinho por padrão: a chave é do projeto e o `--key` é explícito,
  igual ao `forge build`.
"""

__version__ = "0.1.0"
