## Language name (this catalog's own autonym, used to build the language picker)

language-name = Português

## Tray menu (src/modules/tray.rs)

tray-title = COSMIC Wallpaper
tray-open-settings = Abrir configurações...
tray-quit-engine = Encerrar o motor

## First-run desktop integration (src/modules/gui/bootstrap.rs)

general-launcher-issue = O aplicativo ainda não está registrado no menu de aplicativos da sua área de trabalho. Isso normalmente é configurado automaticamente na primeira vez que as Configurações são executadas; se ainda estiver faltando, verifique se ~/.local/share/applications tem permissão de escrita e reinicie as Configurações.

## Status line (src/modules/gui/mod.rs) - the caption at the bottom of every page

status-ready = Pronto.
status-blocked-unsafe-theme-name = Nome de tema bloqueado por segurança: { $name }
status-saved-theme-live = { $name } salvo - a área de trabalho já mostra a alteração.
status-saved-theme-inactive = { $name } salvo. Aplique-o para ver na área de trabalho.
status-error-saving-theme = Erro ao salvar o tema: { $error }
status-error-serialising-theme = Erro ao serializar o tema: { $error }
status-nothing-usable-dropped = Nada utilizável foi solto - apenas arquivos MP4 ou WebM.
status-importing-files = { $count ->
    [one] Importando { $count } arquivo...
   *[other] Importando { $count } arquivos...
}
status-detecting-location = Detectando localização...
status-location-detected = Localização detectada.
status-could-not-detect-location = Não foi possível detectar a localização: { $error }
status-no-videos-imported = Nenhum vídeo importado - apenas arquivos MP4, WebM, MKV, MOV e AVI são compatíveis.
status-imported-videos = { $n ->
    [one] { $n } vídeo importado.
   *[other] { $n } vídeos importados.
}
status-imported-skipped = { $n } importado(s), { $s } ignorado(s) (não são arquivos de vídeo).
status-invalid-theme = Tema inválido: { $error }
status-imported-themes = { $imported ->
    [one] { $imported } tema importado.
   *[other] { $imported } temas importados.
}
status-nothing-imported-themes = Nada foi importado - solte arquivos de tema .toml.
status-engine-starting = Iniciando o motor...
status-failed-to-start-engine = Falha ao iniciar o motor: { $error }
status-could-not-find-engine-binary = Não foi possível encontrar o executável cosmic-wallpaper ao lado das Configurações.
status-killed-by-signal = encerrado por um sinal
status-exit-code = código de saída { $code }
status-engine-exited-immediately = O motor foi encerrado imediatamente ({ $code }).
status-engine-exited-immediately-detail = O motor foi encerrado imediatamente ({ $code }): { $headline }
status-engine-running = O motor está em execução.
status-engine-did-not-start = O motor não iniciou - verifique os registros.
status-engine-stopping = Encerrando o motor...
status-engine-still-running = O motor ainda está em execução.
status-engine-stopped = Motor encerrado.
status-error-applying-theme = Erro ao aplicar o tema: { $error }
status-applied-theme = Tema aplicado: '{ $theme }'
status-select-theme-to-apply = Selecione um tema para aplicar.
status-created-theme = { $file_name } criado
status-theme-already-exists = O tema '{ $name }' já existe!
status-error-creating-theme = Erro ao criar o tema: { $error }
status-fetching-patch-notes = Obtendo notas da versão...
status-xdg-open-not-found = Não foi possível abrir o link: xdg-open não encontrado
status-xdg-open-folder-not-found = Não foi possível abrir a pasta: xdg-open não encontrado
status-diagnostics-copied = Diagnóstico copiado para a área de transferência.
status-downloading = Baixando { $tag }...
status-update-failed = Falha na atualização: { $error }
status-updated-restart-needed = Atualizado para { $tag }! O motor de papel de parede foi reiniciado automaticamente; reinicie também as Configurações para usar a nova versão.
status-select-theme-to-export = Selecione um tema para exportar.
status-exported-pack = Pacote exportado para { $path }
status-error-exporting-pack = Erro ao exportar o pacote: { $error }
status-pack-too-large = '{ $name }' é grande demais para ser um pacote válido (limite de { $limit } MB) - ignorado.
status-could-not-read-dropped-file = Não foi possível ler um dos arquivos soltos.
status-skip-one-shader-at-a-time = '{ $name }' ignorado - importe apenas um pacote com shader por vez.
status-pack-renamed-on-import = '{ $original }' já existia - importado como '{ $written }'.
status-pack-import-error = '{ $name }': { $error }
status-not-a-valid-pack = Pacote inválido: { $error }
status-imported-packs = { $imported ->
    [one] { $imported } pacote importado.
   *[other] { $imported } pacotes importados.
}
status-pack-includes-shader-review = Um pacote inclui um shader personalizado - revise-o acima.
status-nothing-imported-packs = Nada foi importado - solte arquivos de pacote .cwtheme.
status-pack-detail-video-shader = com vídeo e um shader personalizado
status-pack-detail-shader = com um shader personalizado
status-imported-pack-named = Pacote '{ $name }' importado ({ $detail }).
status-pack-renamed-with-detail = '{ $name }' já existia - importado como '{ $written }' ({ $detail }).
status-error-importing-pack = Erro ao importar o pacote: { $error }
status-cancelled-nothing-imported = Cancelado - nada foi importado.
status-applied-pack-with-video = Pacote '{ $name }' aplicado, junto com seu vídeo de fundo.
status-applied-pack-video-missing = Pacote '{ $name }' aplicado - o vídeo de fundo está ausente; reimporte o pacote para restaurá-lo.
status-applied-pack-theme-missing = Pacote '{ $name }' aplicado - o arquivo de tema está ausente, então valores padrão genéricos foram usados.
status-applied-pack = Pacote '{ $name }' aplicado.
status-error-applying-pack = Erro ao aplicar o pacote: { $error }

## Shared across multiple pages (src/modules/gui/view.rs)

common-active = Ativo
common-active-now = Ativo agora.
common-align-center = Centro
common-align-left = Esquerda
common-align-right = Direita
common-apply = Aplicar
common-copied-to-clipboard = Copiado para a área de transferência
common-copy = Copiar
common-copy-to-clipboard = Copiar para a área de transferência
common-create = Criar
common-hide = Ocultar
common-open-folder = Abrir pasta
common-recent-colours = Cores recentes
common-reset = Redefinir
common-retry = Tentar novamente
common-shape-circular = Circular
common-shape-linear = Linear
common-shape-square = Quadrada
common-show = Mostrar
common-start = Iniciar
common-stop = Parar

## Wallpaper page

wallpaper-mode-frosted-glass = Vidro fosco
wallpaper-mode-transparent = Transparente
wallpaper-mode-album-art = Capa do álbum
wallpaper-mode-album-colour = Cor do álbum
wallpaper-mode-live-wallpaper = Papel de parede animado
text-color-mode-automatic = Automático
text-color-mode-custom = Personalizado
wallpaper-preview-none = Nenhum
wallpaper-preview-sample-title = E continua, e continua, e continua, e continua
wallpaper-preview-sample-caption = Consigo sentir a adrenalina, consigo sentir o barulho
wallpaper-style-title = Estilo
wallpaper-frosted-glass-title = Vidro fosco
wallpaper-blur-amount = Intensidade do desfoque
wallpaper-blur-amount-desc = O quanto o papel de parede é desfocado.
wallpaper-live-wallpaper-title = Papel de parede animado
wallpaper-video-item = Vídeo
wallpaper-video-item-desc = Escolha e gerencie vídeos na página Papéis de parede animados
wallpaper-text-title = Texto
wallpaper-text-colour = Cor do texto
wallpaper-text-colour-desc = O modo automático escolhe uma cor que permanece legível sobre o seu papel de parede.
wallpaper-page-title = Papel de parede
wallpaper-page-summary = As opções do estilo selecionado aparecem abaixo dele.

## Live Wallpapers page

live-wallpapers-drop-release = Solte para adicionar à sua biblioteca
live-wallpapers-drop-prompt = Solte arquivos de vídeo aqui para adicioná-los (MP4, WebM)
live-wallpapers-library-title = Biblioteca
live-wallpapers-no-videos = Ainda não há vídeos
live-wallpapers-no-videos-desc = Solte arquivos acima ou use Abrir pasta para adicioná-los manualmente
live-wallpapers-playback-title = Reprodução
live-wallpapers-prefer-canvas = Preferir o Canvas do Spotify
live-wallpapers-prefer-canvas-desc = Quando a faixa em reprodução tiver um loop Canvas, mostrá-lo em vez do papel de parede.
live-wallpapers-library-folder = Pasta da biblioteca
live-wallpapers-library-folder-desc = Os vídeos ficam em ~/.config/cosmic-wallpaper/videos.
live-wallpapers-page-title = Papéis de parede animados
live-wallpapers-page-summary = Vídeos em loop reproduzidos como plano de fundo. Clique em uma miniatura para defini-la.

## Themes page

theme-element-album-art = Capa do álbum
theme-element-track-info = Informações da faixa
theme-element-lyrics = Letra
theme-element-visualiser = Visualizador
theme-element-weather = Clima
theme-element-effects = Efeitos
theme-align = Alinhamento
theme-position-x = Posição X
theme-position-y = Posição Y
theme-size = Tamanho
theme-text-size = Tamanho do texto
theme-shape = Formato
theme-docked = Encaixada
theme-docked-desc = Enquanto a música toca, a capa fica dentro do visualizador circular e acompanha sua posição e tamanho. Desative o encaixe na aba Visualizador para usar estes controles.
theme-band-order = Ordem das bandas
theme-rotation = Rotação
theme-amplitude = Amplitude
theme-bar-width = Largura das barras
theme-cap-roundness = Arredondamento das pontas
theme-glow = Brilho
theme-reflection = Reflexo
theme-led-segments = Segmentos de LED
theme-peak-hold = Retenção de pico
theme-peak-hold-desc = peak_hold - uma pequena ponta brilhante que retém o pico recente de cada barra e depois cai por gravidade
theme-dock-art = Encaixar a capa no anel
theme-dock-art-desc = dock_art - a capa acompanha a posição e o tamanho do anel
theme-lyric-bounce = Quique da letra
theme-spring-stiffness = Rigidez da mola
theme-spring-damping = Amortecimento da mola
theme-beat-pulse = Pulso na batida
theme-reset-section = Redefinir esta seção
theme-reset-section-desc = Restaura { $element } para os valores padrão.
theme-page-theme-title = Tema
theme-editing = Edição
theme-editing-live-desc = Este tema está ativo - as alterações aparecem na sua área de trabalho conforme você as faz.
theme-editing-inactive-desc = Não é o tema ativo - as alterações são salvas no arquivo dele; aplique para vê-las.
theme-manage-title = Gerenciar
theme-create-new = Criar novo tema
theme-create-new-desc = Começa a partir de um modelo completo e totalmente comentado.
theme-name-placeholder = Nome do tema
theme-name-empty-error = O nome do tema não pode estar vazio
theme-name-exists-error = Já existe um tema com este nome
theme-import = Importar
theme-import-desc = Solte arquivos de tema .toml em qualquer lugar desta página para adicioná-los.
theme-page-title = Temas de layout
theme-page-summary = Onde cada elemento fica na tela. Arraste algo e veja sua área de trabalho acompanhar.

## Packs page

packs-your-packs-title = Seus pacotes
packs-none-yet = Ainda não há pacotes importados
packs-none-yet-desc = Solte um arquivo .cwtheme aqui embaixo para vê-lo aparecer.
packs-includes-video-active = Inclui um vídeo de fundo - ativo agora.
packs-includes-video = Inclui um vídeo de fundo.
packs-layout-only = Somente layout.
packs-export-title = Exportar
packs-theme-to-bundle = Tema a agrupar
packs-export-desc-with-video = Agrupa o layout deste tema, seu shader personalizado se houver, e o vídeo de fundo atualmente ativo ({ $file }) - vídeos não são vinculados a um tema específico, então verifique se este é realmente o que você quer compartilhar.
packs-export-desc-no-video = Agrupa o layout deste tema e seu shader personalizado, se houver. Nenhum vídeo de fundo está ativo no momento, então o pacote não incluirá nenhum.
packs-export-pack = Exportar pacote
packs-folder = Pasta de pacotes
packs-folder-desc = Os arquivos .cwtheme exportados ficam em ~/.config/cosmic-wallpaper/packs.
packs-import = Importar
packs-import-desc = Solte um arquivo .cwtheme em qualquer lugar desta página para adicioná-lo.
packs-page-title = Pacotes
packs-page-summary = Compartilhe um visual completo - layout, vídeo de fundo e um shader de visualizador personalizado - em um único arquivo.

## Now Playing page

now-playing-album-art-title = Capa do álbum
now-playing-show-album-art = Mostrar capa do álbum
now-playing-show-album-art-desc = A capa atual, posicionada de acordo com o tema de layout ativo.
now-playing-lyrics-text-title = Letra e texto
now-playing-show-lyrics = Mostrar letra
now-playing-show-lyrics-desc = Letra sincronizada da faixa atual, quando disponível.
now-playing-font-family = Família da fonte
now-playing-page-title = Tocando agora
now-playing-page-summary = O que aparece durante a reprodução de música: capa do álbum, informações da faixa e letra.

## Visualiser page

visualiser-audio-response-title = Resposta de áudio
visualiser-bands = Bandas
visualiser-bands-desc = Quantas barras o visualizador desenha.
visualiser-smoothing = Suavização
visualiser-smoothing-desc = Mais alto é mais calmo; mais baixo é mais responsivo.
visualiser-page-title = Visualizador
visualiser-page-summary = Barras que se movem conforme o que estiver tocando.

## Weather page

weather-unit-celsius = Celsius
weather-unit-fahrenheit = Fahrenheit
weather-poll-5min = 5 minutos
weather-poll-15min = 15 minutos
weather-poll-30min = 30 minutos
weather-poll-1hour = 1 hora
weather-page-title = Clima
weather-show-weather = Mostrar o clima
weather-show-weather-desc = Condições atuais na área de trabalho.
weather-hide-effects = Ocultar efeitos animados
weather-hide-effects-desc = Desativa as animações de chuva e neve para economizar energia.
weather-units = Unidades
weather-location = Localização
weather-location-desc = Latitude e longitude para a previsão. "Usar minha localização" as estima a partir do seu endereço IP via ipapi.co.
weather-latitude-placeholder = Latitude
weather-longitude-placeholder = Longitude
weather-use-my-location = Usar minha localização
weather-update-every = Atualizar a cada
weather-page-summary = Condições e efeitos sobrepostos ao papel de parede.

## General page

general-checking-for-updates = Procurando atualizações...
general-up-to-date = Atualizado
general-check-for-updates = Procurar atualizações
general-check-failed = Não foi possível verificar: { $reason }
general-update-to = Atualizar para { $tag }
general-release-page = Página da versão { $tag }
general-updating-to = Atualizando para { $tag }...
general-installed-restart = { $tag } instalado - reinicie o aplicativo
general-engine-title = Motor
general-wallpaper-engine = Motor do papel de parede
general-engine-running = Em execução (pid { $pid }).
general-engine-not-running = Não está em execução.
general-start-on-login = Iniciar ao entrar na sessão
general-start-on-login-desc = Inicia o motor do papel de parede quando você entra na sessão.
general-frame-rate-limit = Limite de taxa de quadros
general-frame-rate-limit-desc = Um valor mais baixo economiza energia; o motor fica ocioso quando nada está animado.
general-config-folder = Pasta de configuração
general-config-folder-desc = Toda a configuração do motor fica aqui.
general-about-title = Sobre
general-version = Versão
general-patch-notes = Notas da versão
general-patch-notes-desc = O que mudou na última versão.
general-diagnostics = Diagnóstico
general-diagnostics-desc = Versão, trecho de registros e informações da GPU, prontos para colar em um relatório.
general-something-broken = Algo quebrado?
general-something-broken-desc = Abre um relatório de erro pré-preenchido com os erros recentes anexados.
general-report-an-issue = Relatar um problema
general-setup-title = Configuração
general-not-in-launcher = Não está no menu de aplicativos
general-patch-notes-section-title = Notas da versão
general-page-title = Geral
general-page-summary = Comportamento do motor e manutenção.

general-language-title = Idioma
general-language-desc = Substitui o idioma da área de trabalho apenas para este aplicativo - útil para um idioma que sua área de trabalho ainda não oferece.
general-language-system-default = Padrão do sistema
