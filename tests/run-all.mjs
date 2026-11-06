#!/usr/bin/env node
// Roda todos os testes em um único processo.
//
// Por que não `node --test tests/`: o runner do node:test spawna um processo
// por arquivo, e sob o sandbox de arquivos isso falha com `spawn EPERM`.
import "./validate-app.test.mjs";
import "./verificar-frame.test.mjs";
import "./capacidades-input.test.mjs";
import "./catalogo.test.mjs";
import "./verificar-mapa.test.mjs";
