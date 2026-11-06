/*
 * verificador-ed25519 — confere a assinatura de um `.giochi` no aparelho.
 *
 *     verificador-ed25519 <pacote.giochi> <pacote.giochi.sig> <chave.pem>
 *
 * Saída: uma linha no stdout. Código de saída:
 *   0  assinatura confere
 *   1  assinatura NÃO confere (pacote adulterado, ou chave errada)
 *   2  erro de uso ou de I/O (não é veredito sobre o pacote)
 *   3  a chave não é Ed25519
 *
 * ## Por que um binário, e não `openssl`
 *
 * Medido nos rootfs de 04/11/2026 (X55 e X35H, com `debugfs`):
 *
 *     /usr/bin/openssl        AUSENTE nas duas imagens
 *     /usr/bin/sha256sum      presente (busybox) — por isso o hash já é conferido
 *     /usr/lib/libcrypto.so.3 presente, 4,9 MB
 *     python3                 AUSENTE
 *
 * A biblioteca está lá; o programa não. Verificar Ed25519 sem `openssl` exige
 * um binário — e `libcrypto` é exatamente a peça que falta transformar em
 * programa. A alternativa seria pedir `openssl` na imagem: são ~4 MB a mais,
 * quase tudo o que `libcrypto` já ocupa, e a CLI traria uma superfície enorme
 * (comandos de chave, TLS, `s_client`) para uma operação de 40 linhas.
 *
 * ## Por que `EVP_DigestVerify` de uma vez
 *
 * Ed25519 é assinatura de mensagem inteira, não de hash: o `-rawin` do
 * `openssl pkeyutl` e a API `EVP_DigestVerify*` fazem a mesma coisa. O `.sig`
 * gerado pelo empacotador tem 64 bytes crus, que é o que esta API consome —
 * nenhuma conversão de formato, nenhum base64.
 *
 * ## O que este programa NÃO resolve
 *
 * Ele confia na chave que recebe. A pergunta "esta chave é a da Giochi?" é do
 * **ancoramento de confiança**, e ela mora na imagem: se a chave puder ser
 * trocada junto com o pacote (num cartão SD, por exemplo), a verificação não
 * vale nada. Lê-la de `/etc/giochi/loja-publica.pem` é decisão de quem monta a
 * imagem, não deste programa — por isso o caminho é argumento.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <openssl/bio.h>
#include <openssl/evp.h>
#include <openssl/pem.h>

/* Lê o arquivo inteiro. Devolve NULL e explica em `erro` quando falha. */
static unsigned char *ler_tudo(const char *caminho, long *tamanho, const char **erro) {
    FILE *f = fopen(caminho, "rb");
    if (f == NULL) {
        *erro = "não consegui abrir";
        return NULL;
    }
    if (fseek(f, 0, SEEK_END) != 0) {
        *erro = "não consegui medir";
        fclose(f);
        return NULL;
    }
    long n = ftell(f);
    if (n < 0) {
        *erro = "não consegui medir";
        fclose(f);
        return NULL;
    }
    rewind(f);
    unsigned char *dados = malloc((size_t)n == 0 ? 1 : (size_t)n);
    if (dados == NULL) {
        *erro = "sem memória";
        fclose(f);
        return NULL;
    }
    if (n > 0 && fread(dados, 1, (size_t)n, f) != (size_t)n) {
        *erro = "leitura incompleta";
        free(dados);
        fclose(f);
        return NULL;
    }
    fclose(f);
    *tamanho = n;
    return dados;
}

int main(int argc, char **argv) {
    if (argc != 4) {
        printf("USO verificador-ed25519 <pacote> <pacote.sig> <chave.pem>\n");
        return 2;
    }

    const char *erro = NULL;
    long tamanho_pkg = 0, tamanho_sig = 0;

    unsigned char *pkg = ler_tudo(argv[1], &tamanho_pkg, &erro);
    if (pkg == NULL) {
        printf("ERRO pacote: %s\n", erro);
        return 2;
    }
    unsigned char *sig = ler_tudo(argv[2], &tamanho_sig, &erro);
    if (sig == NULL) {
        printf("ERRO assinatura: %s\n", erro);
        free(pkg);
        return 2;
    }

    /*
     * O tamanho é conferido antes de verificar, e não depois.
     *
     * Ed25519 tem 64 bytes fixos. Um `.sig` com 89 bytes é assinatura em base64
     * gravada como texto — erro que já aconteceu neste projeto, e que
     * `EVP_DigestVerify` recusaria com uma mensagem que não aponta a causa.
     */
    if (tamanho_sig != 64) {
        printf("ERRO assinatura com %ld bytes; Ed25519 tem 64\n", tamanho_sig);
        free(pkg);
        free(sig);
        return 1;
    }

    BIO *bio = BIO_new_file(argv[3], "r");
    if (bio == NULL) {
        printf("ERRO chave: não consegui abrir\n");
        free(pkg);
        free(sig);
        return 2;
    }
    EVP_PKEY *chave = PEM_read_bio_PUBKEY(bio, NULL, NULL, NULL);
    BIO_free(bio);
    if (chave == NULL) {
        printf("ERRO chave: não é uma chave pública PEM\n");
        free(pkg);
        free(sig);
        return 2;
    }

    /*
     * Confere o **tipo**. Uma chave RSA passaria em `EVP_DigestVerifyInit` e
     * falharia na verificação, e o veredito sairia "assinatura não confere" —
     * culpando o pacote por um erro de configuração. O usuário trocaria o jogo
     * em vez da chave.
     */
    if (EVP_PKEY_id(chave) != EVP_PKEY_ED25519) {
        printf("ERRO chave não é Ed25519 (tipo %d)\n", EVP_PKEY_id(chave));
        EVP_PKEY_free(chave);
        free(pkg);
        free(sig);
        return 3;
    }

    int veredito = 1;
    EVP_MD_CTX *ctx = EVP_MD_CTX_new();
    if (ctx == NULL) {
        printf("ERRO sem memória para o contexto\n");
    } else if (EVP_DigestVerifyInit(ctx, NULL, NULL, NULL, chave) != 1) {
        printf("ERRO não consegui iniciar a verificação\n");
    } else {
        /* De uma vez só: Ed25519 assina a mensagem, não um hash dela. */
        int r = EVP_DigestVerify(ctx, sig, (size_t)tamanho_sig,
                                 pkg, (size_t)tamanho_pkg);
        if (r == 1) {
            printf("OK assinatura confere (%ld bytes)\n", tamanho_pkg);
            veredito = 0;
        } else if (r == 0) {
            printf("RECUSADO assinatura não confere — pacote adulterado ou chave errada\n");
            veredito = 1;
        } else {
            printf("ERRO falha ao verificar\n");
            veredito = 2;
        }
        EVP_MD_CTX_free(ctx);
    }

    EVP_PKEY_free(chave);
    free(pkg);
    free(sig);
    return veredito;
}
