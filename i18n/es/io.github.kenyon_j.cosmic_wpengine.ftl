## Language name (this catalog's own autonym, used to build the language picker)

language-name = Español

## Tray menu (src/modules/tray.rs)

tray-title = COSMIC Wallpaper
tray-open-settings = Abrir ajustes...
tray-quit-engine = Salir del motor

## First-run desktop integration (src/modules/gui/bootstrap.rs)

general-launcher-issue = La aplicación aún no está registrada en el lanzador de tu escritorio. Esto normalmente se configura de forma automática la primera vez que se ejecuta Ajustes; si sigue sin aparecer, comprueba que ~/.local/share/applications tenga permisos de escritura y reinicia Ajustes.

## Status line (src/modules/gui/mod.rs) - the caption at the bottom of every page

status-ready = Listo.
status-blocked-unsafe-theme-name = Nombre de tema bloqueado por seguridad: { $name }
status-saved-theme-live = Se guardó { $name }: el escritorio ya muestra el cambio.
status-saved-theme-inactive = Se guardó { $name }. Aplícalo para verlo en el escritorio.
status-error-saving-theme = Error al guardar el tema: { $error }
status-error-serialising-theme = Error al serializar el tema: { $error }
status-nothing-usable-dropped = No se soltó nada utilizable: solo archivos MP4 o WebM.
status-importing-files = { $count ->
    [one] Importando { $count } archivo...
   *[other] Importando { $count } archivos...
}
status-detecting-location = Detectando ubicación...
status-location-detected = Ubicación detectada.
status-could-not-detect-location = No se pudo detectar la ubicación: { $error }
status-no-videos-imported = No se importó ningún vídeo: solo se admiten archivos MP4, WebM, MKV, MOV y AVI.
status-imported-videos = { $n ->
    [one] Se importó { $n } vídeo.
   *[other] Se importaron { $n } vídeos.
}
status-imported-skipped = Se importaron { $n }; se omitieron { $s } (no son archivos de vídeo).
status-invalid-theme = Tema no válido: { $error }
status-imported-themes = { $imported ->
    [one] Se importó { $imported } tema.
   *[other] Se importaron { $imported } temas.
}
status-nothing-imported-themes = No se importó nada: suelta archivos de tema .toml.
status-engine-starting = Iniciando el motor...
status-failed-to-start-engine = No se pudo iniciar el motor: { $error }
status-could-not-find-engine-binary = No se encontró el binario cosmic-wallpaper junto a Ajustes.
status-killed-by-signal = terminado por una señal
status-exit-code = código de salida { $code }
status-engine-exited-immediately = El motor se cerró inmediatamente ({ $code }).
status-engine-exited-immediately-detail = El motor se cerró inmediatamente ({ $code }): { $headline }
status-engine-running = El motor está en ejecución.
status-engine-did-not-start = El motor no se inició; revisa los registros.
status-engine-stopping = Deteniendo el motor...
status-engine-still-running = El motor sigue en ejecución.
status-engine-stopped = El motor se detuvo.
status-error-applying-theme = Error al aplicar el tema: { $error }
status-applied-theme = Tema aplicado: «{ $theme }»
status-select-theme-to-apply = Selecciona un tema para aplicar.
status-created-theme = Se creó { $file_name }
status-theme-already-exists = El tema «{ $name }» ya existe.
status-error-creating-theme = Error al crear el tema: { $error }
status-fetching-patch-notes = Obteniendo las notas de la versión...
status-xdg-open-not-found = No se pudo abrir el enlace: no se encontró xdg-open
status-xdg-open-folder-not-found = No se pudo abrir la carpeta: no se encontró xdg-open
status-diagnostics-copied = Diagnóstico copiado al portapapeles.
status-downloading = Descargando { $tag }...
status-update-failed = Error al actualizar: { $error }
status-updated-restart-needed = Se actualizó a { $tag }. El motor del fondo de pantalla se reinició automáticamente; reinicia Ajustes para usar también la nueva versión.
status-select-theme-to-export = Selecciona un tema para exportar.
status-exported-pack = Paquete exportado a { $path }
status-error-exporting-pack = Error al exportar el paquete: { $error }
status-pack-too-large = «{ $name }» es demasiado grande para ser un paquete válido (límite de { $limit } MB); se omitió.
status-could-not-read-dropped-file = No se pudo leer uno de los archivos soltados.
status-skip-one-shader-at-a-time = Se omitió «{ $name }»: importa un paquete con shader a la vez.
status-pack-renamed-on-import = «{ $original }» ya existía; se importó como «{ $written }».
status-pack-import-error = «{ $name }»: { $error }
status-not-a-valid-pack = El paquete no es válido: { $error }
status-imported-packs = { $imported ->
    [one] Se importó { $imported } paquete.
   *[other] Se importaron { $imported } paquetes.
}
status-pack-includes-shader-review = Un paquete incluye un shader personalizado; revísalo arriba.
status-nothing-imported-packs = No se importó nada: suelta archivos de paquete .cwtheme.
status-pack-detail-video-shader = con vídeo y un shader personalizado
status-pack-detail-shader = con un shader personalizado
status-imported-pack-named = Se importó el paquete «{ $name }» ({ $detail }).
status-pack-renamed-with-detail = «{ $name }» ya existía; se importó como «{ $written }» ({ $detail }).
status-error-importing-pack = Error al importar el paquete: { $error }
status-cancelled-nothing-imported = Cancelado: no se importó nada.
status-applied-pack-with-video = Se aplicó el paquete «{ $name }» junto con su vídeo de fondo.
status-applied-pack-video-missing = Se aplicó el paquete «{ $name }»: falta su vídeo de fondo; vuelve a importar el paquete para restaurarlo.
status-applied-pack-theme-missing = Se aplicó el paquete «{ $name }»: falta su archivo de tema, así que se usaron valores predeterminados genéricos.
status-applied-pack = Se aplicó el paquete «{ $name }».
status-error-applying-pack = Error al aplicar el paquete: { $error }

## Shared across multiple pages (src/modules/gui/view.rs)

common-active = Activo
common-active-now = Activo ahora.
common-align-center = Centro
common-align-left = Izquierda
common-align-right = Derecha
common-apply = Aplicar
common-copied-to-clipboard = Copiado al portapapeles
common-copy = Copiar
common-copy-to-clipboard = Copiar al portapapeles
common-create = Crear
common-hide = Ocultar
common-open-folder = Abrir carpeta
common-recent-colours = Colores recientes
common-reset = Restablecer
common-retry = Reintentar
common-shape-circular = Circular
common-shape-linear = Lineal
common-shape-square = Cuadrada
common-show = Mostrar
common-start = Iniciar
common-stop = Detener

## Wallpaper page

wallpaper-mode-frosted-glass = Cristal esmerilado
wallpaper-mode-transparent = Transparente
wallpaper-mode-album-art = Carátula del álbum
wallpaper-mode-album-colour = Color del álbum
wallpaper-mode-live-wallpaper = Fondo de pantalla en vivo
text-color-mode-automatic = Automático
text-color-mode-custom = Personalizado
wallpaper-preview-none = Ninguno
wallpaper-preview-sample-title = Y sigue, y sigue, y sigue, y sigue
wallpaper-preview-sample-caption = Puedo sentir la adrenalina, puedo sentir el ruido
wallpaper-style-title = Estilo
wallpaper-frosted-glass-title = Cristal esmerilado
wallpaper-blur-amount = Intensidad del desenfoque
wallpaper-blur-amount-desc = Cuánto se desenfoca el fondo de pantalla.
wallpaper-live-wallpaper-title = Fondo de pantalla en vivo
wallpaper-video-item = Vídeo
wallpaper-video-item-desc = Elige y administra vídeos en la página Fondos en vivo
wallpaper-text-title = Texto
wallpaper-text-colour = Color del texto
wallpaper-text-colour-desc = El modo automático elige un color que se mantiene legible sobre tu fondo de pantalla.
wallpaper-page-title = Fondo de pantalla
wallpaper-page-summary = Debajo del estilo elegido aparecen sus opciones.

## Live Wallpapers page

live-wallpapers-drop-release = Suelta para añadirlo a tu biblioteca
live-wallpapers-drop-prompt = Suelta aquí archivos de vídeo para añadirlos (MP4, WebM)
live-wallpapers-library-title = Biblioteca
live-wallpapers-no-videos = Aún no hay vídeos
live-wallpapers-no-videos-desc = Suelta archivos arriba, o usa Abrir carpeta para añadirlos manualmente
live-wallpapers-playback-title = Reproducción
live-wallpapers-prefer-canvas = Preferir Canvas de Spotify
live-wallpapers-prefer-canvas-desc = Cuando la canción en reproducción tenga un bucle Canvas, mostrarlo en lugar del fondo de pantalla.
live-wallpapers-library-folder = Carpeta de la biblioteca
live-wallpapers-library-folder-desc = Los vídeos se guardan en ~/.config/cosmic-wallpaper/videos.
live-wallpapers-page-title = Fondos de pantalla en vivo
live-wallpapers-page-summary = Vídeos en bucle que se reproducen como fondo. Haz clic en una miniatura para activarla.

## Themes page

theme-element-album-art = Carátula del álbum
theme-element-track-info = Información de la canción
theme-element-lyrics = Letra
theme-element-visualiser = Visualizador
theme-element-weather = Clima
theme-element-effects = Efectos
theme-align = Alineación
theme-position-x = Posición X
theme-position-y = Posición Y
theme-size = Tamaño
theme-text-size = Tamaño del texto
theme-shape = Forma
theme-docked = Acoplado
theme-docked-desc = Mientras suena música, la carátula se acopla dentro del visualizador circular y sigue su posición y tamaño. Desactiva el acoplamiento en la pestaña Visualizador para usar estos controles.
theme-band-order = Orden de las bandas
theme-rotation = Rotación
theme-amplitude = Amplitud
theme-bar-width = Ancho de las barras
theme-cap-roundness = Redondez de las puntas
theme-glow = Resplandor
theme-reflection = Reflejo
theme-led-segments = Segmentos LED
theme-peak-hold = Puntas de pico
theme-peak-hold-desc = peak_hold: una pequeña punta brillante que retiene el pico reciente de cada barra y cae por gravedad
theme-dock-art = Acoplar la carátula en el anillo
theme-dock-art-desc = dock_art: la carátula sigue la posición y el tamaño del anillo
theme-lyric-bounce = Rebote de la letra
theme-spring-stiffness = Rigidez del resorte
theme-spring-damping = Amortiguación del resorte
theme-beat-pulse = Pulso al ritmo
theme-reset-section = Restablecer esta sección
theme-reset-section-desc = Restaura { $element } a sus valores predeterminados.
theme-page-theme-title = Tema
theme-editing = Edición
theme-editing-live-desc = Este tema está activo: los cambios aparecen en tu escritorio a medida que los haces.
theme-editing-inactive-desc = No es el tema activo: los cambios se guardan en su archivo; aplícalo para verlos.
theme-manage-title = Administrar
theme-create-new = Crear tema nuevo
theme-create-new-desc = Comienza a partir de una plantilla completa con comentarios.
theme-name-placeholder = Nombre del tema
theme-name-empty-error = El nombre del tema no puede estar vacío
theme-name-exists-error = Ya existe un tema con este nombre
theme-import = Importar
theme-import-desc = Suelta archivos de tema .toml en cualquier parte de esta página para añadirlos.
theme-page-title = Temas de diseño
theme-page-summary = Dónde se ubica todo en la pantalla. Desliza algo y observa cómo lo sigue tu escritorio.

## Packs page

packs-your-packs-title = Tus paquetes
packs-none-yet = Aún no hay paquetes importados
packs-none-yet-desc = Suelta aquí abajo un archivo .cwtheme para verlo en esta lista.
packs-includes-video-active = Incluye un vídeo de fondo; activo ahora.
packs-includes-video = Incluye un vídeo de fondo.
packs-layout-only = Solo diseño.
packs-export-title = Exportar
packs-theme-to-bundle = Tema a incluir
packs-export-desc-with-video = Incluye el diseño de este tema, su shader personalizado si lo tiene, y tu vídeo de fondo actualmente activo ({ $file }); los vídeos no están vinculados a un tema en concreto, así que comprueba que sea el que quieres compartir.
packs-export-desc-no-video = Incluye el diseño de este tema y su shader personalizado, si lo tiene. No hay ningún vídeo de fondo activo, así que el paquete no incluirá ninguno.
packs-export-pack = Exportar paquete
packs-folder = Carpeta de paquetes
packs-folder-desc = Los archivos .cwtheme exportados se guardan en ~/.config/cosmic-wallpaper/packs.
packs-import = Importar
packs-import-desc = Suelta un archivo .cwtheme en cualquier parte de esta página para añadirlo.
packs-page-title = Paquetes
packs-page-summary = Comparte un aspecto completo (diseño, vídeo de fondo y un shader de visualizador personalizado) en un solo archivo.

## Now Playing page

now-playing-album-art-title = Carátula del álbum
now-playing-show-album-art = Mostrar carátula del álbum
now-playing-show-album-art-desc = La portada actual, ubicada según el tema de diseño activo.
now-playing-lyrics-text-title = Letra y texto
now-playing-show-lyrics = Mostrar letra
now-playing-show-lyrics-desc = Letra sincronizada de la canción actual, cuando esté disponible.
now-playing-font-family = Tipo de letra
now-playing-page-title = Reproduciendo ahora
now-playing-page-summary = Lo que aparece cuando suena música: carátula del álbum, información de la canción y letra.

## Visualiser page

visualiser-audio-response-title = Respuesta al audio
visualiser-bands = Bandas
visualiser-bands-desc = Cuántas barras dibuja el visualizador.
visualiser-smoothing = Suavizado
visualiser-smoothing-desc = Más alto es más calmado; más bajo es más ágil.
visualiser-page-title = Visualizador
visualiser-page-summary = Barras que se mueven con lo que esté sonando.

## Weather page

weather-unit-celsius = Celsius
weather-unit-fahrenheit = Fahrenheit
weather-poll-5min = 5 minutos
weather-poll-15min = 15 minutos
weather-poll-30min = 30 minutos
weather-poll-1hour = 1 hora
weather-page-title = Clima
weather-show-weather = Mostrar el clima
weather-show-weather-desc = Condiciones actuales en el escritorio.
weather-hide-effects = Ocultar efectos animados
weather-hide-effects-desc = Desactiva las animaciones de lluvia y nieve para ahorrar energía.
weather-units = Unidades
weather-location = Ubicación
weather-location-desc = Latitud y longitud para el pronóstico. «Usar mi ubicación» las estima a partir de tu dirección IP mediante ipapi.co.
weather-latitude-placeholder = Latitud
weather-longitude-placeholder = Longitud
weather-use-my-location = Usar mi ubicación
weather-update-every = Actualizar cada
weather-page-summary = Condiciones y efectos superpuestos sobre el fondo de pantalla.

## General page

general-checking-for-updates = Buscando actualizaciones...
general-up-to-date = Actualizado
general-check-for-updates = Buscar actualizaciones
general-check-failed = No se pudo comprobar: { $reason }
general-update-to = Actualizar a { $tag }
general-release-page = Página de la versión { $tag }
general-updating-to = Actualizando a { $tag }...
general-installed-restart = { $tag } instalado: reinicia la aplicación
general-engine-title = Motor
general-wallpaper-engine = Motor del fondo de pantalla
general-engine-running = En ejecución (pid { $pid }).
general-engine-not-running = No se está ejecutando.
general-start-on-login = Iniciar al iniciar sesión
general-start-on-login-desc = Inicia el motor del fondo de pantalla cuando inicias sesión.
general-frame-rate-limit = Límite de fotogramas
general-frame-rate-limit-desc = Un valor más bajo ahorra energía; el motor está inactivo cuando nada se anima.
general-config-folder = Carpeta de configuración
general-config-folder-desc = Aquí se guarda toda la configuración del motor.
general-about-title = Acerca de
general-version = Versión
general-patch-notes = Notas de la versión
general-patch-notes-desc = Qué cambió en la última versión.
general-diagnostics = Diagnóstico
general-diagnostics-desc = Versión, fragmento de registro e información de la GPU, listos para pegar en un informe.
general-something-broken = ¿Algo no funciona?
general-something-broken-desc = Abre un informe de error prellenado con los últimos errores adjuntos.
general-report-an-issue = Reportar un problema
general-setup-title = Configuración
general-not-in-launcher = No está en el lanzador de aplicaciones
general-patch-notes-section-title = Notas de la versión
general-page-title = General
general-page-summary = Comportamiento del motor y mantenimiento.

general-language-title = Idioma
general-language-desc = Anula el idioma del escritorio solo para esta aplicación; útil para un idioma que tu escritorio aún no ofrece.
general-language-system-default = Predeterminado del sistema
