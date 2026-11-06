/* Arcade Volleyball - port jogavel do classico de 1988 para o Giochi Konsol OS.
 *
 * Arquivo unico, C++17 + SDL2, sem fonte e sem audio: tudo e desenhado com o
 * SDL_Renderer (retangulos, linhas e uma bola feita de spans horizontais).
 * Alvo: os portateis RK3326/RK3566 do projeto, entrada por gamepad evdev.
 *
 * A fisica roda numa resolucao logica fixa (1280x720) e o
 * SDL_RenderSetLogicalSize escala para o painel real - 640x480 na X35S/X35H,
 * 1280x720 na X55 - sem nenhuma constante do jogo depender do aparelho.
 *
 * Controles:
 *   teclado   A/D (ou setas) movem, W/espaco pulam, Esc ou Q saem
 *   gamepad   dpad esquerda/direita movem, A pula, Start sai
 *
 * O P2 e uma IA simples: o console nao tem um segundo controle.
 *
 * Build: make -C "Jogos/Arcade Volleyball"   (SDL2 via pkg-config)
 */

#include <algorithm>
#include <array>
#include <atomic>
#include <cerrno>
#include <chrono>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

#include <fcntl.h>
#include <unistd.h>

#include <SDL.h>

namespace {

constexpr int kWindowWidth = 1280;
constexpr int kWindowHeight = 720;
constexpr int kGroundHeight = 50;
constexpr int kFps = 60;
constexpr float kGravity = 0.6f;

/* --- orientação do painel ------------------------------------------------
 *
 * O jogo desenha em espaço de projeto (1280x720, paisagem). O X35H serve essa
 * paisagem como está (painel 640x480, `fbcon=rotate:0`); o X55 tem painel
 * **720x1280 retrato** com `fbcon=rotate:3`, e sem girar o desenho o conteúdo
 * sai espremido nos primeiros 480 px do buffer.
 *
 * A matemática é a de `app/jogos/volley/src/main.rs`, que por sua vez veio de
 * `app/launcher/crates/ui-sdl/src/draw.rs`. Duplicar é deliberado: são apps
 * diferentes e um não pode reclamar do outro quando quebrar.
 */
enum class Rotacao { Nenhuma, Cw90, Ccw90 };

Rotacao rotacao_do_sistema()
{
    if (const char *texto = std::getenv("GIOCHI_ROTATE")) {
        if (std::strcmp(texto, "0") == 0 || std::strcmp(texto, "none") == 0) {
            return Rotacao::Nenhuma;
        }
        if (std::strcmp(texto, "90") == 0 || std::strcmp(texto, "cw") == 0) {
            return Rotacao::Cw90;
        }
        if (std::strcmp(texto, "270") == 0 || std::strcmp(texto, "ccw") == 0) {
            return Rotacao::Ccw90;
        }
    }

    std::FILE *arquivo = std::fopen("/sys/class/graphics/fbcon/rotate", "r");
    if (arquivo == nullptr) {
        return Rotacao::Nenhuma;
    }
    char buffer[8] = {0};
    const std::size_t lido = std::fread(buffer, 1, sizeof(buffer) - 1, arquivo);
    std::fclose(arquivo);
    if (lido == 0) {
        return Rotacao::Nenhuma;
    }
    /* `rotate=2` (180°) não é suportado em lugar nenhum do projeto: nenhum
     * painel precisa, e fingir que atendeu seria pior que avisar. */
    if (buffer[0] == '1') {
        return Rotacao::Cw90;
    }
    if (buffer[0] == '3') {
        return Rotacao::Ccw90;
    }
    return Rotacao::Nenhuma;
}

/* Converte entre o espaço de projeto do jogo e o painel.
 *
 * Duas coisas acontecem aqui, e separá-las é o que evita o bug: a **escala**
 * (o jogo é 1280x720; um painel 640x480 precisa de 0,5) e a **rotação**. Um
 * retângulo do jogo vira retângulo no painel passando pelos dois.
 */
struct Tela {
    Rotacao rotacao = Rotacao::Nenhuma;
    int painel_largura = kWindowWidth;
    int painel_altura = kWindowHeight;
    int design_largura = kWindowWidth;
    int design_altura = kWindowHeight;
    float escala = 1.0f;
    int offset_x = 0;
    int offset_y = 0;

    void configurar(int largura, int altura, Rotacao rot)
    {
        rotacao = rot;
        painel_largura = largura;
        painel_altura = altura;
        if (rot == Rotacao::Nenhuma) {
            design_largura = largura;
            design_altura = altura;
        } else {
            design_largura = altura;
            design_altura = largura;
        }
        const float kx = static_cast<float>(design_largura) / kWindowWidth;
        const float ky = static_cast<float>(design_altura) / kWindowHeight;
        escala = std::min(kx, ky);
        const int usado_x = static_cast<int>(std::lround(kWindowWidth * escala));
        const int usado_y = static_cast<int>(std::lround(kWindowHeight * escala));
        offset_x = (design_largura - usado_x) / 2;
        offset_y = (design_altura - usado_y) / 2;
    }

    /* Ponto do jogo (1280x720) → ponto do painel. */
    void ponto(float jx, float jy, int *px, int *py) const
    {
        const float sx = jx * escala + static_cast<float>(offset_x);
        const float sy = jy * escala + static_cast<float>(offset_y);
        switch (rotacao) {
        case Rotacao::Cw90:
            *px = static_cast<int>(std::lround(design_altura - sy));
            *py = static_cast<int>(std::lround(sx));
            break;
        case Rotacao::Ccw90:
            *px = static_cast<int>(std::lround(sy));
            *py = static_cast<int>(std::lround(design_largura - sx));
            break;
        case Rotacao::Nenhuma:
        default:
            *px = static_cast<int>(std::lround(sx));
            *py = static_cast<int>(std::lround(sy));
            break;
        }
    }

    /* Mapear os dois cantos e reordenar resolve as duas rotações com uma
     * fórmula só — e um retângulo alinhado aos eixos continua alinhado. */
    SDL_Rect retangulo(int x, int y, int largura, int altura) const
    {
        int x0 = 0;
        int y0 = 0;
        int x1 = 0;
        int y1 = 0;
        ponto(static_cast<float>(x), static_cast<float>(y), &x0, &y0);
        ponto(static_cast<float>(x + largura), static_cast<float>(y + altura), &x1, &y1);
        SDL_Rect ret;
        ret.x = std::min(x0, x1);
        ret.y = std::min(y0, y1);
        ret.w = std::max(1, std::abs(x1 - x0));
        ret.h = std::max(1, std::abs(y1 - y0));
        return ret;
    }
};

/* --- entrada por evdev ---------------------------------------------------
 *
 * NÃO se usa `SDL_GameController` aqui, embora o port original usasse. Os
 * botões dos handhelds do projeto vêm de driver próprio (gpio-keys /
 * rocknix-joypad) e o SDL só reconhece controle que esteja no
 * `SDL_GameControllerDB` — que não tem entrada para aparelho custom. Resultado
 * no aparelho: `SDL_GameControllerOpen` devolve nullptr e **nenhum botão
 * funciona**, sem erro visível.
 *
 * É a mesma decisão do launcher (`input_evdev.rs`) e do volley.
 */
constexpr std::uint16_t kEvKey = 1;
constexpr std::size_t kEvEventSize = 24; /* struct input_event em 64 bits */
constexpr std::size_t kEvMaxCodigo = 768; /* KEY_MAX = 0x2ff */

/* Códigos do `buttons.map` do aparelho. O mapa é do sistema; traduzir é do app,
 * então os números ficam aqui — igual ao volley. */
constexpr std::uint16_t kBtnSouth = 304;   /* A      */
constexpr std::uint16_t kBtnEast = 305;    /* B      */
constexpr std::uint16_t kBtnSelect = 314;  /* Select */
constexpr std::uint16_t kBtnStart = 315;   /* Start  */
constexpr std::uint16_t kBtnDpadUp = 544;
constexpr std::uint16_t kBtnDpadDown = 545;
constexpr std::uint16_t kBtnDpadLeft = 546;
constexpr std::uint16_t kBtnDpadRight = 547;

class EntradaEvdev {
public:
    explicit EntradaEvdev(std::string rotulo) : rotulo_(std::move(rotulo))
    {
        for (auto &aberto : aberto_) {
            aberto = false;
        }
    }

    ~EntradaEvdev() { parar(); }

    EntradaEvdev(const EntradaEvdev &) = delete;
    EntradaEvdev &operator=(const EntradaEvdev &) = delete;

    void iniciar()
    {
        for (int i = 0; i < 32; ++i) {
            char caminho[40];
            std::snprintf(caminho, sizeof(caminho), "/dev/input/event%d", i);

            const int fd = ::open(caminho, O_RDONLY | O_NONBLOCK);
            if (fd < 0) {
                continue;
            }
            /* Um dispositivo que abre pode não entregar nada (por exemplo o
             * powerkey). Não custa nada: cada um é uma thread. */
            threads_.emplace_back([this, fd]() { ler(fd); });
            ++abertos_;
        }

        if (abertos_ == 0) {
            std::fprintf(stderr,
                         "%s: nenhum /dev/input/event* abriu — use o teclado\n",
                         rotulo_.c_str());
        } else {
            std::fprintf(stderr, "%s: %d dispositivo(s) de entrada abertos\n",
                         rotulo_.c_str(), abertos_);
        }
    }

    void parar()
    {
        if (parando_.exchange(true)) {
            return;
        }
        for (int fd : descritores_) {
            ::close(fd);
        }
        descritores_.clear();
        for (auto &t : threads_) {
            if (t.joinable()) {
                t.join();
            }
        }
        threads_.clear();
    }

    bool apertada(std::uint16_t codigo) const
    {
        if (codigo >= kEvMaxCodigo) {
            return false;
        }
        const std::lock_guard<std::mutex> trava(mutex_);
        return pressionada_[codigo];
    }

    /* Borda: consome, para que segurar o botão não repique. */
    bool consumir_borda(std::uint16_t codigo)
    {
        if (codigo >= kEvMaxCodigo) {
            return false;
        }
        const std::lock_guard<std::mutex> trava(mutex_);
        if (!pressionada_[codigo] || borda_consumida_[codigo]) {
            return false;
        }
        borda_consumida_[codigo] = true;
        return true;
    }

    int abertos() const { return abertos_; }

private:
    void ler(int fd)
    {
        {
            const std::lock_guard<std::mutex> trava(mutex_);
            descritores_.push_back(fd);
        }

        std::array<unsigned char, kEvEventSize> buffer{};
        while (!parando_.load()) {
            const ssize_t lido = ::read(fd, buffer.data(), buffer.size());
            if (lido < static_cast<ssize_t>(kEvEventSize)) {
                if (lido < 0 && (errno == EAGAIN || errno == EWOULDBLOCK)) {
                    std::this_thread::sleep_for(std::chrono::milliseconds(4));
                    continue;
                }
                break;
            }

            const std::uint16_t tipo =
                static_cast<std::uint16_t>(buffer[16] | (buffer[17] << 8));
            if (tipo != kEvKey) {
                continue;
            }
            const std::uint16_t codigo =
                static_cast<std::uint16_t>(buffer[18] | (buffer[19] << 8));
            const std::int32_t valor = static_cast<std::int32_t>(
                buffer[20] | (buffer[21] << 8) | (buffer[22] << 16) |
                (static_cast<std::uint32_t>(buffer[23]) << 24));

            if (codigo >= kEvMaxCodigo) {
                continue;
            }
            const std::lock_guard<std::mutex> trava(mutex_);
            /* valor 2 é auto-repeat: o estado já está pressionado. */
            if (valor == 0) {
                pressionada_[codigo] = false;
                borda_consumida_[codigo] = false;
            } else {
                pressionada_[codigo] = true;
            }
        }
    }

    std::string rotulo_;
    mutable std::mutex mutex_;
    std::array<bool, kEvMaxCodigo> pressionada_{};
    std::array<bool, kEvMaxCodigo> borda_consumida_{};
    std::vector<int> descritores_;
    std::vector<std::thread> threads_;
    std::atomic<bool> parando_{false};
    std::array<bool, 32> aberto_{};
    int abertos_ = 0;
};

/* --- argumentos de linha de comando ------------------------------------- */

struct Opcoes {
    int largura = 0; /* 0 = painel inteiro (fullscreen) */
    int altura = 0;
    const char *dump_frame = nullptr;
    int quadros = 0; /* 0 = para quando o jogador sair */
};

bool tamanho_pedido(const char *texto, int *largura, int *altura)
{
    if (texto == nullptr) {
        return false;
    }
    int l = 0;
    int a = 0;
    if (std::sscanf(texto, "%dx%d", &l, &a) != 2 && std::sscanf(texto, "%dX%d", &l, &a) != 2) {
        return false;
    }
    if (l <= 0 || a <= 0) {
        return false;
    }
    *largura = l;
    *altura = a;
    return true;
}

Opcoes ler_opcoes(int argc, char **argv)
{
    Opcoes opcoes;
    for (int i = 1; i < argc; ++i) {
        const char *arg = argv[i];
        if (std::strcmp(arg, "--tamanho") == 0 && i + 1 < argc) {
            tamanho_pedido(argv[++i], &opcoes.largura, &opcoes.altura);
        } else if (std::strcmp(arg, "--dump-frame") == 0 && i + 1 < argc) {
            opcoes.dump_frame = argv[++i];
        } else if (std::strcmp(arg, "--quadros") == 0 && i + 1 < argc) {
            opcoes.quadros = std::atoi(argv[++i]);
        } else if (std::strcmp(arg, "--ajuda") == 0) {
            std::printf(
                "arcade-volleyball — vôlei de arquivo\n"
                "\n"
                "  --tamanho LxA      pede um tamanho de janela (só faz sentido fora\n"
                "                     do aparelho; com SDL_VIDEODRIVER=dummy vale sempre)\n"
                "  --dump-frame ARQ   grava o quadro atual em BMP ao sair\n"
                "  --quadros N        sai sozinho depois de N quadros\n"
                "\n"
                "GIOCHI_ROTATE=0|90|270 sobrepõe a orientação do sistema.\n");
            std::exit(0);
        }
    }
    return opcoes;
}

constexpr int kFloorY = kWindowHeight - kGroundHeight;

/* Paleta do tema dark_default (assets/themes/dark_default.json): nenhuma cor inventada. */
constexpr SDL_Color kColorBackground = {15, 15, 15, 255};   /* #0F0F0F background */
constexpr SDL_Color kColorGround = {30, 30, 30, 255};       /* #1E1E1E surface */
constexpr SDL_Color kColorNet = {51, 51, 51, 255};          /* #333333 divider */
constexpr SDL_Color kColorPlayer1 = {229, 9, 20, 255};      /* #E50914 accent */
constexpr SDL_Color kColorPlayer2 = {160, 160, 160, 255};   /* #A0A0A0 text_secondary */
constexpr SDL_Color kColorBall = {229, 229, 229, 255};      /* #E5E5E5 text_primary */

struct Player {
    float x = 0.0f;
    float y = 0.0f;
    float vx = 0.0f;
    float vy = 0.0f;
    int width = 80;
    int height = 100;
    float speed = 8.0f;
    float jump_force = -14.0f;
    bool grounded = true;
    int score = 0;

    void place(float start_x)
    {
        x = start_x;
        y = static_cast<float>(kFloorY - height);
        vx = 0.0f;
        vy = 0.0f;
        grounded = true;
    }

    float center_x() const { return x + static_cast<float>(width) / 2.0f; }
};

struct Ball {
    float x = 0.0f;
    float y = 0.0f;
    float vx = 0.0f;
    float vy = 0.0f;
    int radius = 20;
    float max_speed = 15.0f;

    /* Saque do lado de quem fez o ponto, indo em direcao ao outro lado. */
    void serve(bool from_left)
    {
        x = from_left ? kWindowWidth * 0.25f : kWindowWidth * 0.75f;
        y = kWindowHeight * 0.3f;
        vx = from_left ? 4.0f : -4.0f;
        vy = 0.0f;
    }
};

struct Net {
    int x = 0;
    int y = 0;
    int width = 16;
    int height = 250;
};

class ArcadeVolleyball {
public:
    ArcadeVolleyball() = default;
    ArcadeVolleyball(const ArcadeVolleyball &) = delete;
    ArcadeVolleyball &operator=(const ArcadeVolleyball &) = delete;
    ~ArcadeVolleyball() { shutdown(); }

    bool init(const Opcoes &opcoes)
    {
        opcoes_ = opcoes;

        if (SDL_Init(SDL_INIT_VIDEO) != 0) {
            std::fprintf(stderr, "arcade-volleyball: SDL_Init: %s\n", SDL_GetError());
            return false;
        }
        sdl_ready_ = true;

        /* No aparelho o painel tem tamanho próprio e o KMSDRM manda nele: pedir
         * 1280x720 num painel 720x1280 produz letterbox ou recusa. Fora dele,
         * `--tamanho` pede o que o teste precisa. */
        const bool pedido = opcoes.largura > 0 && opcoes.altura > 0;
        Uint32 bandeiras = SDL_WINDOW_SHOWN;
        if (!pedido) {
            bandeiras |= SDL_WINDOW_FULLSCREEN_DESKTOP;
        }

        window_ = SDL_CreateWindow("Arcade Volleyball - Giochi OS",
                                   SDL_WINDOWPOS_CENTERED, SDL_WINDOWPOS_CENTERED,
                                   pedido ? opcoes.largura : kWindowWidth,
                                   pedido ? opcoes.altura : kWindowHeight,
                                   bandeiras);
        if (window_ == nullptr) {
            std::fprintf(stderr, "arcade-volleyball: SDL_CreateWindow: %s\n",
                         SDL_GetError());
            return false;
        }

        renderer_ = SDL_CreateRenderer(window_, -1, SDL_RENDERER_ACCELERATED |
                                                      SDL_RENDERER_PRESENTVSYNC);
        if (renderer_ == nullptr) {
            /* Sem aceleracao (framebuffer puro, driver dummy) ainda da para
             * jogar: cai para o renderer por software. */
            renderer_ = SDL_CreateRenderer(window_, -1, 0);
        }
        if (renderer_ == nullptr) {
            std::fprintf(stderr, "arcade-volleyball: SDL_CreateRenderer: %s\n",
                         SDL_GetError());
            return false;
        }

        /* Sem `SDL_RenderSetLogicalSize`: ele escala mas **não gira**, e num
         * painel retrato o resultado é o jogo deitado dentro de uma tela em pé.
         * O espaço lógico passa a ser o do painel e a transformação (escala +
         * rotação) é feita em `Tela`. */
        int largura = 0;
        int altura = 0;
        (void)SDL_GetRendererOutputSize(renderer_, &largura, &altura);
        tela_.configurar(largura, altura, rotacao_do_sistema());

        const char *nome_rotacao = "Nenhuma";
        if (tela_.rotacao == Rotacao::Cw90) {
            nome_rotacao = "Cw90";
        } else if (tela_.rotacao == Rotacao::Ccw90) {
            nome_rotacao = "Ccw90";
        }
        SDL_RendererInfo info;
        const char *nome_renderer = "?";
        if (SDL_GetRendererInfo(renderer_, &info) == 0 && info.name != nullptr) {
            nome_renderer = info.name;
        }
        std::fprintf(stderr,
                     "arcade-volleyball: painel %dx%d, projeto %dx%d (%s), "
                     "escala %.2f, renderer %s\n",
                     tela_.painel_largura, tela_.painel_altura,
                     tela_.design_largura, tela_.design_altura, nome_rotacao,
                     static_cast<double>(tela_.escala), nome_renderer);

        entrada_.iniciar();

        p1_.place(kWindowWidth * 0.2f - 40.0f);
        p2_.place(kWindowWidth * 0.8f - 40.0f);
        net_.width = 16;
        net_.height = 250;
        net_.x = kWindowWidth / 2 - net_.width / 2;
        net_.y = kFloorY - net_.height;
        ball_.serve(true);
        return true;
    }

    void run()
    {
        constexpr Uint32 kFrameDelay = 1000 / kFps;

        while (running_) {
            const Uint32 frame_start = SDL_GetTicks();

            handle_events();
            update();
            render();

            if (opcoes_.quadros > 0 && ++quadros_ >= opcoes_.quadros) {
                running_ = false;
            }

            const Uint32 elapsed = SDL_GetTicks() - frame_start;
            if (elapsed < kFrameDelay) {
                SDL_Delay(kFrameDelay - elapsed);
            }
        }

        if (opcoes_.dump_frame != nullptr) {
            salvar_quadro(opcoes_.dump_frame);
        }
        shutdown();
    }

private:
    void shutdown()
    {
        entrada_.parar();
        if (renderer_ != nullptr) {
            SDL_DestroyRenderer(renderer_);
            renderer_ = nullptr;
        }
        if (window_ != nullptr) {
            SDL_DestroyWindow(window_);
            window_ = nullptr;
        }
        if (sdl_ready_) {
            SDL_Quit();
            sdl_ready_ = false;
        }
    }

    /* Grava o quadro em BMP.
     *
     * Existe para poder afirmar "desenhou" sem olhar a tela — a mesma razão do
     * `--dump-frame` do launcher e do volley. `read_pixels` lê em coordenadas
     * de dispositivo, e é isso que se quer aqui: o BMP é a prova crua de que a
     * transformação (escala + rotação) produziu o quadro certo. */
    void salvar_quadro(const char *destino)
    {
        if (renderer_ == nullptr || destino == nullptr) {
            return;
        }
        int largura = 0;
        int altura = 0;
        if (SDL_GetRendererOutputSize(renderer_, &largura, &altura) != 0 ||
            largura <= 0 || altura <= 0) {
            std::fprintf(stderr, "arcade-volleyball: output: %s\n", SDL_GetError());
            return;
        }

        SDL_Surface *superficie = SDL_CreateRGBSurfaceWithFormat(
            0, largura, altura, 32, SDL_PIXELFORMAT_ARGB8888);
        if (superficie == nullptr) {
            std::fprintf(stderr, "arcade-volleyball: superficie: %s\n", SDL_GetError());
            return;
        }
        if (SDL_RenderReadPixels(renderer_, nullptr, SDL_PIXELFORMAT_ARGB8888,
                                 superficie->pixels, superficie->pitch) != 0) {
            std::fprintf(stderr, "arcade-volleyball: read_pixels: %s\n", SDL_GetError());
            SDL_FreeSurface(superficie);
            return;
        }
        if (SDL_SaveBMP(superficie, destino) != 0) {
            std::fprintf(stderr, "arcade-volleyball: bmp: %s\n", SDL_GetError());
        } else {
            std::fprintf(stderr, "arcade-volleyball: quadro %dx%d gravado em %s\n",
                         largura, altura, destino);
        }
        SDL_FreeSurface(superficie);
    }

    /* --- entrada ------------------------------------------------------ */

    /* Os botões do aparelho são lidos por evdev, não por `SDL_GameController`:
     * ver o comentário de `EntradaEvdev`. O teclado continua pelo SDL, para
     * testar no PC. */
    void handle_events()
    {
        SDL_Event event;
        while (SDL_PollEvent(&event) != 0) {
            switch (event.type) {
            case SDL_QUIT:
                running_ = false;
                break;
            case SDL_KEYDOWN:
                if (event.key.repeat != 0) {
                    break; /* auto-repeat do X nao deve re-disparar o pulo */
                }
                switch (event.key.keysym.sym) {
                case SDLK_ESCAPE:
                case SDLK_q:
                    running_ = false;
                    break;
                case SDLK_a:
                case SDLK_LEFT:
                    p1_left_ = true;
                    break;
                case SDLK_d:
                case SDLK_RIGHT:
                    p1_right_ = true;
                    break;
                case SDLK_w:
                case SDLK_UP:
                case SDLK_SPACE:
                    p1_jump_ = true; /* borda: consumida em update() */
                    break;
                default:
                    break;
                }
                break;
            case SDL_KEYUP:
                switch (event.key.keysym.sym) {
                case SDLK_a:
                case SDLK_LEFT:
                    p1_left_ = false;
                    break;
                case SDLK_d:
                case SDLK_RIGHT:
                    p1_right_ = false;
                    break;
                default:
                    break;
                }
                break;
            case SDL_CONTROLLERDEVICEADDED:
            case SDL_CONTROLLERDEVICEREMOVED:
            case SDL_CONTROLLERBUTTONDOWN:
            case SDL_CONTROLLERBUTTONUP:
                /* Deliberadamente ignorados: a entrada do aparelho é evdev.
                 * Se o SDL chegasse a reconhecer o controle, ele processaria o
                 * mesmo botão duas vezes. */
                break;
            default:
                break;
            }
        }
    }

    /* --- simulacao ---------------------------------------------------- */

    void update()
    {
        /* Junta as duas fontes de entrada aqui, num lugar só: teclado (SDL) e
         * controle (evdev). Sem isso cada `update_*` teria de conhecer as duas,
         * e a primeira que esquecesse um dos lados ficaria muda no aparelho. */
        if (entrada_.consumir_borda(kBtnSouth) || entrada_.consumir_borda(kBtnEast)) {
            p1_jump_ = true;
        }
        if (entrada_.consumir_borda(kBtnStart) || entrada_.consumir_borda(kBtnSelect)) {
            running_ = false;
            std::fprintf(stderr, "arcade-volleyball: saída pedida pelo controle\n");
        }

        update_player1();
        update_player2();
        update_ball();
        p1_jump_ = false; /* pulo e de borda: segurar o botao nao repica */
    }

    void update_player1()
    {
        const bool esquerda = p1_left_ || entrada_.apertada(kBtnDpadLeft);
        const bool direita = p1_right_ || entrada_.apertada(kBtnDpadRight);

        if (esquerda && !direita) {
            p1_.vx = -p1_.speed;
        } else if (direita && !esquerda) {
            p1_.vx = p1_.speed;
        } else {
            p1_.vx = 0.0f;
        }

        if (p1_jump_ && p1_.grounded) {
            p1_.vy = p1_.jump_force;
            p1_.grounded = false;
        }

        p1_.vy += kGravity;
        p1_.x += p1_.vx;
        p1_.y += p1_.vy;

        if (p1_.x < 0.0f) {
            p1_.x = 0.0f;
        }
        const float left_limit = static_cast<float>(net_.x - p1_.width);
        if (p1_.x > left_limit) {
            p1_.x = left_limit;
        }

        const float floor_line = static_cast<float>(kFloorY - p1_.height);
        if (p1_.y >= floor_line) {
            p1_.y = floor_line;
            p1_.vy = 0.0f;
            p1_.grounded = true;
        }
    }

    void update_player2()
    {
        const float net_right = static_cast<float>(net_.x + net_.width);

        if (ball_.x > net_right) {
            const float delta = ball_.x - p2_.center_x();
            if (delta < -10.0f) {
                p2_.vx = -p2_.speed * 0.8f;
            } else if (delta > 10.0f) {
                p2_.vx = p2_.speed * 0.8f;
            } else {
                p2_.vx = 0.0f;
            }

            if (ball_.y < p2_.y && p2_.grounded &&
                std::fabs(ball_.x - p2_.center_x()) < 50.0f) {
                p2_.vy = p2_.jump_force;
                p2_.grounded = false;
            }
        } else {
            /* Volta para a posicao de espera enquanto a bola esta do outro lado. */
            const float home_x = kWindowWidth * 0.75f - static_cast<float>(p2_.width) / 2.0f;
            if (p2_.x < home_x - 10.0f) {
                p2_.vx = p2_.speed * 0.5f;
            } else if (p2_.x > home_x + 10.0f) {
                p2_.vx = -p2_.speed * 0.5f;
            } else {
                p2_.vx = 0.0f;
            }
        }

        p2_.vy += kGravity;
        p2_.x += p2_.vx;
        p2_.y += p2_.vy;

        if (p2_.x < net_right) {
            p2_.x = net_right;
        }
        if (p2_.x + p2_.width > kWindowWidth) {
            p2_.x = static_cast<float>(kWindowWidth - p2_.width);
        }

        const float floor_line = static_cast<float>(kFloorY - p2_.height);
        if (p2_.y >= floor_line) {
            p2_.y = floor_line;
            p2_.vy = 0.0f;
            p2_.grounded = true;
        }
    }

    void update_ball()
    {
        const float radius = static_cast<float>(ball_.radius);

        ball_.vy += kGravity * 0.7f;
        ball_.x += ball_.vx;
        ball_.y += ball_.vy;

        if (ball_.x - radius < 0.0f) {
            ball_.x = radius;
            ball_.vx = -ball_.vx;
            nudge_vertical();
        }
        if (ball_.x + radius > kWindowWidth) {
            ball_.x = static_cast<float>(kWindowWidth) - radius;
            ball_.vx = -ball_.vx;
            nudge_vertical();
        }
        if (ball_.y - radius < 0.0f) {
            ball_.y = radius;
            ball_.vy = -ball_.vy;
        }

        collide_player(p1_);
        collide_player(p2_);
        collide_net();

        /* Uma colisao pode empurrar a bola para fora da tela por um quadro
         * (jogador encostado na parede): recoloca dentro antes de desenhar. */
        ball_.x = std::clamp(ball_.x, radius,
                             static_cast<float>(kWindowWidth) - radius);
        ball_.y = std::max(ball_.y, radius);

        if (ball_.y + radius > static_cast<float>(kFloorY)) {
            if (ball_.x < static_cast<float>(net_.x)) {
                /* Caiu no lado do P1: ponto do P2. */
                ++p2_.score;
                ball_.serve(false);
            } else {
                ++p1_.score;
                ball_.serve(true);
            }
            std::fprintf(stdout, "arcade-volleyball: %d x %d\n",
                         p1_.score, p2_.score);
        }
    }

    /* A rede e uma parede: a bola so passa por cima do topo. Sem isso ela
     * atravessaria a rede e o ponto sairia do lado errado. */
    void collide_net()
    {
        const float radius = static_cast<float>(ball_.radius);
        const float net_left = static_cast<float>(net_.x);
        const float net_right = static_cast<float>(net_.x + net_.width);

        if (ball_.y + radius <= static_cast<float>(net_.y)) {
            return; /* passou por cima da rede */
        }
        if (ball_.x + radius <= net_left || ball_.x - radius >= net_right) {
            return; /* fora da faixa horizontal da rede */
        }

        if (ball_.vx > 0.0f) {
            ball_.x = net_left - radius;
            ball_.vx = -ball_.vx;
        } else if (ball_.vx < 0.0f) {
            ball_.x = net_right + radius;
            ball_.vx = -ball_.vx;
        } else {
            /* Bola parada em cima da fita: empurra para o lado mais proximo e
             * deixa a gravidade resolver (evita travar a partida). */
            ball_.x = (ball_.x < static_cast<float>(net_.x) + net_.width / 2.0f)
                          ? net_left - radius
                          : net_right + radius;
        }
    }

    void collide_player(const Player &player)
    {
        const float radius = static_cast<float>(ball_.radius);
        const float left = player.x;
        const float right = player.x + static_cast<float>(player.width);
        const float top = player.y;
        const float bottom = player.y + static_cast<float>(player.height);

        const float closest_x = std::clamp(ball_.x, left, right);
        const float closest_y = std::clamp(ball_.y, top, bottom);
        const float dx = ball_.x - closest_x;
        const float dy = ball_.y - closest_y;
        const float distance_squared = dx * dx + dy * dy;

        /* distance_squared == 0 significa centro da bola dentro do jogador:
         * sai pela borda no quadro seguinte, nao ha normal para refletir. */
        if (distance_squared <= 0.0f || distance_squared >= radius * radius) {
            return;
        }

        const float distance = std::sqrt(distance_squared);
        const float nx = dx / distance;
        const float ny = dy / distance;

        ball_.x = closest_x + nx * radius;
        ball_.y = closest_y + ny * radius;

        /* Quique quase vertical (bola batendo na cabeca do jogador, nx ~ 0)
         * cairia sempre na mesma vertical: o jogador se reposiciona e a bola
         * quica de novo na cabeca dele, sem nunca cruzar a rede (partida
         * travada). Nesse caso o desvio sai para o lado, afastando-se do
         * centro do jogador, com a componente horizontal cheia - o jogador
         * nao tem velocidade para acompanhar. */
        float kick_x = nx * ball_.max_speed;
        float kick_y = ny * ball_.max_speed;
        if (std::fabs(nx) < 0.2f) {
            const float side = (ball_.x < player.center_x()) ? -1.0f : 1.0f;
            kick_x = side * ball_.max_speed;
            if (kick_y < 0.0f) {
                kick_y *= 0.6f; /* sobe menos: a saida e lateral */
            }
        }

        ball_.vx = std::clamp(kick_x + player.vx * 0.5f,
                              -ball_.max_speed, ball_.max_speed);
        ball_.vy = std::clamp(kick_y + player.vy * 0.5f,
                              -ball_.max_speed, ball_.max_speed);

        /* A velocidade do jogador pode cancelar o empurrao lateral (bola
         * sobe reta e volta para a mesma cabeca). O toque lateral garante
         * que sempre exista componente horizontal. */
        nudge_vertical();
    }

    /* Quique praticamente vertical: a bola sobe e desce no mesmo x para
     * sempre. Devolve um empurrao lateral, como no port Godot
     * (scripts/sim/match.gd, _nudge_vertical). */
    void nudge_vertical()
    {
        if (std::fabs(ball_.vx) >= 0.5f) {
            return;
        }
        ball_.vx = (ball_.x <= kWindowWidth * 0.5f) ? 3.0f : -3.0f;
    }

    /* --- desenho ------------------------------------------------------ */

    void set_color(const SDL_Color &color)
    {
        (void)SDL_SetRenderDrawColor(renderer_, color.r, color.g, color.b, color.a);
    }

    void fill_rect(int x, int y, int width, int height)
    {
        const SDL_Rect rect = tela_.retangulo(x, y, width, height);
        (void)SDL_RenderFillRect(renderer_, &rect);
    }

    void draw_ball()
    {
        const int center_x = static_cast<int>(std::lround(ball_.x));
        const int center_y = static_cast<int>(std::lround(ball_.y));
        const int radius = ball_.radius;

        set_color(kColorBall);
        for (int dy = -radius; dy <= radius; ++dy) {
            const int half = static_cast<int>(
                std::sqrt(static_cast<double>(radius * radius - dy * dy)));
            /* As linhas são horizontais no espaço do jogo: giradas, viram
             * verticais. Mapear as duas pontas cobre os dois casos. */
            int x0 = 0;
            int y0 = 0;
            int x1 = 0;
            int y1 = 0;
            tela_.ponto(static_cast<float>(center_x - half),
                        static_cast<float>(center_y + dy), &x0, &y0);
            tela_.ponto(static_cast<float>(center_x + half),
                        static_cast<float>(center_y + dy), &x1, &y1);
            (void)SDL_RenderDrawLine(renderer_, x0, y0, x1, y1);
        }
    }

    /* Placar sem fonte: digitos de 7 segmentos em retangulos. */
    void draw_digit(int value, int x, int y)
    {
        static constexpr unsigned kSegments[10] = {
            0x3Fu, 0x06u, 0x5Bu, 0x4Fu, 0x66u,
            0x6Du, 0x7Du, 0x07u, 0x7Fu, 0x6Fu,
        };
        constexpr int kWidth = 28;
        constexpr int kHeight = 48;
        constexpr int kThickness = 6;
        constexpr int kSegmentHeight = (kHeight - 3 * kThickness) / 2;

        const unsigned mask = kSegments[value % 10];
        if ((mask & 0x01u) != 0u) { /* a - topo */
            fill_rect(x + kThickness, y, kWidth - 2 * kThickness, kThickness);
        }
        if ((mask & 0x02u) != 0u) { /* b - superior direita */
            fill_rect(x + kWidth - kThickness, y + kThickness, kThickness, kSegmentHeight);
        }
        if ((mask & 0x04u) != 0u) { /* c - inferior direita */
            fill_rect(x + kWidth - kThickness, y + 2 * kThickness + kSegmentHeight,
                      kThickness, kSegmentHeight);
        }
        if ((mask & 0x08u) != 0u) { /* d - base */
            fill_rect(x + kThickness, y + kHeight - kThickness,
                      kWidth - 2 * kThickness, kThickness);
        }
        if ((mask & 0x10u) != 0u) { /* e - inferior esquerda */
            fill_rect(x, y + 2 * kThickness + kSegmentHeight, kThickness, kSegmentHeight);
        }
        if ((mask & 0x20u) != 0u) { /* f - superior esquerda */
            fill_rect(x, y + kThickness, kThickness, kSegmentHeight);
        }
        if ((mask & 0x40u) != 0u) { /* g - meio */
            fill_rect(x + kThickness, y + (kHeight - kThickness) / 2,
                      kWidth - 2 * kThickness, kThickness);
        }
    }

    void draw_number(int value, int x, int y, bool right_aligned)
    {
        constexpr int kDigitWidth = 28;
        constexpr int kDigitGap = 8;

        value = std::clamp(value, 0, 99);
        const int digits = value < 10 ? 1 : 2;
        const int total_width = digits * kDigitWidth + (digits - 1) * kDigitGap;
        int cursor = right_aligned ? x - total_width : x;

        if (digits == 2) {
            draw_digit(value / 10, cursor, y);
            cursor += kDigitWidth + kDigitGap;
        }
        draw_digit(value % 10, cursor, y);
    }

    void draw_score()
    {
        set_color(kColorBall);
        draw_number(p1_.score, kWindowWidth / 2 - 80, 24, true);
        draw_number(p2_.score, kWindowWidth / 2 + 80, 24, false);
    }

    void render()
    {
        set_color(kColorBackground);
        (void)SDL_RenderClear(renderer_);

        set_color(kColorGround);
        fill_rect(0, kFloorY, kWindowWidth, kGroundHeight);

        set_color(kColorNet);
        fill_rect(net_.x, net_.y, net_.width, net_.height);

        set_color(kColorPlayer1);
        fill_rect(static_cast<int>(std::lround(p1_.x)),
                  static_cast<int>(std::lround(p1_.y)), p1_.width, p1_.height);

        set_color(kColorPlayer2);
        fill_rect(static_cast<int>(std::lround(p2_.x)),
                  static_cast<int>(std::lround(p2_.y)), p2_.width, p2_.height);

        draw_ball();
        draw_score();

        SDL_RenderPresent(renderer_);
    }

    bool running_ = true;
    bool sdl_ready_ = false;
    SDL_Window *window_ = nullptr;
    SDL_Renderer *renderer_ = nullptr;

    Tela tela_;
    EntradaEvdev entrada_{"arcade-volleyball"};
    Opcoes opcoes_;
    int quadros_ = 0;

    Player p1_;
    Player p2_;
    Ball ball_;
    Net net_;

    bool p1_left_ = false;
    bool p1_right_ = false;
    bool p1_jump_ = false;
};

} /* namespace */

int main(int argc, char **argv)
{
    const Opcoes opcoes = ler_opcoes(argc, argv);

    ArcadeVolleyball game;
    if (!game.init(opcoes)) {
        return 1;
    }
    game.run();
    return 0;
}
