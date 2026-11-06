// Front do painel. Sem framework e sem build: uma página, três requisições.
//
// A regra que segue: **nada de estado inventado**. Todo o texto vem do servidor,
// que por sua vez chama o forge e o store. Se a página mostrasse um veredito
// calculado no JS, existiria uma segunda regra de compatibilidade — e ela
// envelheceria diferente da primeira.

const $ = (id) => document.getElementById(id);

function status(texto, classe = "") {
  const el = $("status");
  el.textContent = texto;
  el.className = classe;
}

async function api(caminho, opcoes) {
  const resp = await fetch(caminho, opcoes);
  const dados = await resp.json().catch(() => ({ erro: "resposta não é JSON" }));
  if (!resp.ok) throw new Error(dados.erro || `HTTP ${resp.status}`);
  return dados;
}

function escapar(texto) {
  return String(texto).replace(/[&<>"']/g, (c) => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
  })[c]);
}

// ── aparelhos ─────────────────────────────────────────────
async function carregarAparelhos() {
  try {
    const { aparelhos } = await api("/api/aparelhos");
    const sel = $("aparelho");
    sel.innerHTML = aparelhos
      .map((a) => {
        const godot =
          a.godot_runtime === false ? "sem Godot" :
          a.godot_runtime === true ? "com Godot" : "Godot não medido";
        return `<option value="${escapar(a.id)}">${escapar(a.id)} — ${escapar(a.soc)}, ${a.ram_mb} MB, ${escapar(a.display)} (${escapar(godot)})</option>`;
      })
      .join("");
    status("pronto");
  } catch (e) {
    status(`erro: ${e.message}`, "erro-min");
  }
}

// ── classificar ───────────────────────────────────────────
async function classificar() {
  const projeto = $("projeto").value.trim();
  if (!projeto) return status("informe a pasta do app", "erro-min");

  $("btn-classificar").disabled = true;
  status("classificando…");
  try {
    const r = await api("/api/classificar", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ projeto, aparelho: $("aparelho").value }),
    });

    const classe = r.codigo === 0 ? "ok" : r.codigo === 1 ? "parcial" : "erro";
    const problemas = r.problemas.length
      ? `<ul class="problemas">${r.problemas
          .map((p) => `<li><code>${escapar(p.requisito)}</code> — ${escapar(p.detalhe)}</li>`)
          .join("")}</ul>`
      : "";
    const avisos = r.avisos.length
      ? `<ul class="problemas">${r.avisos.map((a) => `<li>aviso: ${escapar(a)}</li>`).join("")}</ul>`
      : "";

    $("veredito").innerHTML = `
      <div class="veredito ${classe}">
        <strong>${escapar(r.veredito)}</strong> em ${escapar(r.aparelho)}
        <div class="dica" style="margin:6px 0 0">requisitos de ${escapar(r.origem)} · ${escapar(r.projeto)}</div>
        ${problemas}${avisos}
      </div>`;
    status("classificado");
  } catch (e) {
    $("veredito").innerHTML = `<div class="veredito erro"><strong>erro</strong><div class="dica" style="margin:6px 0 0">${escapar(e.message)}</div></div>`;
    status("erro", "erro-min");
  } finally {
    $("btn-classificar").disabled = false;
  }
}

// ── empacotar ─────────────────────────────────────────────
async function empacotar() {
  const projeto = $("projeto").value.trim();
  const saida = $("saida").value.trim() || "dist/pacotes";
  if (!projeto) return status("informe a pasta do app", "erro-min");

  $("btn-empacotar").disabled = true;
  status("empacotando…");
  try {
    const r = await api("/api/empacotar", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ projeto, saida, chave: $("chave").value.trim() }),
    });
    $("pacote").innerHTML = `
      <div class="veredito ok">
        <strong>pacote gerado</strong>
        <div class="dica" style="margin:6px 0 0">
          ${escapar(r.arquivo)} · ${r.bytes} bytes · ${r.assinado ? "assinado" : "sem assinatura"}
        </div>
        <div class="mono" style="margin-top:6px;word-break:break-all;color:var(--texto2);font-family:ui-monospace,monospace;font-size:11.5px">${escapar(r.sha256)}</div>
      </div>`;
    status("empacotado");
  } catch (e) {
    $("pacote").innerHTML = `<div class="veredito erro"><strong>erro</strong><div class="dica" style="margin:6px 0 0">${escapar(e.message)}</div></div>`;
    status("erro", "erro-min");
  } finally {
    $("btn-empacotar").disabled = false;
  }
}

// ── catálogo ──────────────────────────────────────────────
async function listarPacotes() {
  const origem = $("origem").value.trim();
  if (!origem) return status("informe a pasta com .giochi", "erro-min");

  $("btn-pacotes").disabled = true;
  status("lendo…");
  try {
    const r = await api(`/api/pacotes?origem=${encodeURIComponent(origem)}`);
    if (!r.pacotes.length) {
      $("lista").innerHTML = `<p class="vazio">nenhum .giochi em ${escapar(r.pasta)}</p>`;
    } else {
      const linhas = r.pacotes
        .map((p) => `
          <tr>
            <td>${escapar(p.titulo)}</td>
            <td>${escapar(p.versao)}</td>
            <td>${escapar(p.runtime)}</td>
            <td class="${p.ok ? "ok-min" : "erro-min"}">${p.ok ? "ok" : escapar(p.motivo || "recusado")}</td>
            <td class="mono">${escapar(p.sha256.slice(0, 12))}…</td>
          </tr>`)
        .join("");
      $("lista").innerHTML = `
        <table>
          <thead><tr><th>app</th><th>versão</th><th>runtime</th><th>estado</th><th>sha256</th></tr></thead>
          <tbody>${linhas}</tbody>
        </table>`;
    }
    status(`${r.pacotes.length} pacote(s)`);
  } catch (e) {
    $("lista").innerHTML = `<p class="erro-min">${escapar(e.message)}</p>`;
    status("erro", "erro-min");
  } finally {
    $("btn-pacotes").disabled = false;
  }
}

// ── destinos ──────────────────────────────────────────────
async function procurarDestinos() {
  $("btn-destinos").disabled = true;
  status("procurando o console…");
  try {
    const r = await api("/api/destinos");
    if (!r.destinos.length) {
      $("destinos").innerHTML = `<p class="vazio">nenhum console visível.
        Coloque o cartão no leitor, ou ligue o aparelho com o cabo USB de dados.</p>`;
    } else {
      $("destinos").innerHTML = `
        <table>
          <thead><tr><th>tipo</th><th>caminho</th></tr></thead>
          <tbody>${r.destinos
            .map((d) => `<tr><td>${escapar(d.tipo)}</td><td class="mono">${escapar(d.caminho)}</td></tr>`)
            .join("")}</tbody>
        </table>`;
    }
    status(`${r.destinos.length} destino(s)`);
  } catch (e) {
    $("destinos").innerHTML = `<p class="erro-min">${escapar(e.message)}</p>`;
    status("erro", "erro-min");
  } finally {
    $("btn-destinos").disabled = false;
  }
}

// ── ligação ───────────────────────────────────────────────
$("btn-classificar").addEventListener("click", classificar);
$("btn-empacotar").addEventListener("click", empacotar);
$("btn-pacotes").addEventListener("click", listarPacotes);
$("btn-destinos").addEventListener("click", procurarDestinos);
// Enter no campo do projeto classifica: é o uso mais frequente.
$("projeto").addEventListener("keydown", (e) => {
  if (e.key === "Enter") classificar();
});
$("origem").addEventListener("keydown", (e) => {
  if (e.key === "Enter") listarPacotes();
});

carregarAparelhos();
