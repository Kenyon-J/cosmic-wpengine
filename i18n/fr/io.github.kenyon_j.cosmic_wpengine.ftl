## Language name (this catalog's own autonym, used to build the language picker)

language-name = Français

## Tray menu (src/modules/tray.rs)

tray-title = COSMIC Wallpaper
tray-open-settings = Ouvrir les paramètres...
tray-quit-engine = Quitter le moteur

## First-run desktop integration (src/modules/gui/bootstrap.rs)

general-launcher-issue = L'application n'est pas encore enregistrée dans le lanceur de votre bureau. Cela se met normalement en place automatiquement au premier lancement de Paramètres ; si le problème persiste, vérifiez que ~/.local/share/applications est accessible en écriture, puis redémarrez Paramètres.

## Status line (src/modules/gui/mod.rs) - the caption at the bottom of every page

status-ready = Prêt.
status-blocked-unsafe-theme-name = Nom de thème bloqué pour des raisons de sécurité : { $name }
status-saved-theme-live = { $name } enregistré : le bureau affiche déjà la modification.
status-saved-theme-inactive = { $name } enregistré. Appliquez-le pour le voir sur le bureau.
status-error-saving-theme = Erreur lors de l'enregistrement du thème : { $error }
status-error-serialising-theme = Erreur lors de la sérialisation du thème : { $error }
status-nothing-usable-dropped = Rien d'exploitable n'a été déposé : seuls les fichiers MP4 ou WebM sont acceptés.
status-importing-files = { $count ->
    [one] Import de { $count } fichier...
   *[other] Import de { $count } fichiers...
}
status-detecting-location = Détection de la position...
status-location-detected = Position détectée.
status-could-not-detect-location = Impossible de détecter la position : { $error }
status-no-videos-imported = Aucune vidéo importée : seuls les fichiers MP4, WebM, MKV, MOV et AVI sont pris en charge.
status-imported-videos = { $n ->
    [one] { $n } vidéo importée.
   *[other] { $n } vidéos importées.
}
status-imported-skipped = { $n } importée(s), { $s } ignorée(s) (fichiers non vidéo).
status-invalid-theme = Thème non valide : { $error }
status-imported-themes = { $imported ->
    [one] { $imported } thème importé.
   *[other] { $imported } thèmes importés.
}
status-nothing-imported-themes = Rien n'a été importé : déposez des fichiers de thème .toml.
status-engine-starting = Démarrage du moteur...
status-failed-to-start-engine = Échec du démarrage du moteur : { $error }
status-could-not-find-engine-binary = Impossible de trouver l'exécutable cosmic-wallpaper à côté de Paramètres.
status-killed-by-signal = arrêté par un signal
status-exit-code = code de sortie { $code }
status-engine-exited-immediately = Le moteur s'est arrêté immédiatement ({ $code }).
status-engine-exited-immediately-detail = Le moteur s'est arrêté immédiatement ({ $code }) : { $headline }
status-engine-running = Le moteur est en cours d'exécution.
status-engine-did-not-start = Le moteur n'a pas démarré : consultez les journaux.
status-engine-stopping = Arrêt du moteur...
status-engine-still-running = Le moteur est toujours en cours d'exécution.
status-engine-stopped = Moteur arrêté.
status-error-applying-theme = Erreur lors de l'application du thème : { $error }
status-applied-theme = Thème appliqué : « { $theme } »
status-select-theme-to-apply = Sélectionnez un thème à appliquer.
status-created-theme = { $file_name } créé
status-theme-already-exists = Le thème « { $name } » existe déjà.
status-error-creating-theme = Erreur lors de la création du thème : { $error }
status-fetching-patch-notes = Récupération des notes de version...
status-xdg-open-not-found = Impossible d'ouvrir le lien : xdg-open est introuvable
status-xdg-open-folder-not-found = Impossible d'ouvrir le dossier : xdg-open est introuvable
status-diagnostics-copied = Diagnostic copié dans le presse-papiers.
status-downloading = Téléchargement de { $tag }...
status-update-failed = Échec de la mise à jour : { $error }
status-updated-restart-needed = Mise à jour vers { $tag } effectuée ! Le moteur de fond d'écran a redémarré automatiquement ; redémarrez également Paramètres pour utiliser la nouvelle version.
status-select-theme-to-export = Sélectionnez un thème à exporter.
status-exported-pack = Pack exporté vers { $path }
status-error-exporting-pack = Erreur lors de l'exportation du pack : { $error }
status-pack-too-large = « { $name } » est trop volumineux pour être un pack valide (limite de { $limit } Mo) : ignoré.
status-could-not-read-dropped-file = Impossible de lire un des fichiers déposés.
status-skip-one-shader-at-a-time = « { $name } » ignoré : n'importez qu'un seul pack contenant un shader à la fois.
status-pack-renamed-on-import = « { $original } » existait déjà : importé sous le nom « { $written } ».
status-pack-import-error = « { $name } » : { $error }
status-not-a-valid-pack = Pack non valide : { $error }
status-imported-packs = { $imported ->
    [one] { $imported } pack importé.
   *[other] { $imported } packs importés.
}
status-pack-includes-shader-review = Un pack contient un shader personnalisé : vérifiez-le ci-dessus.
status-nothing-imported-packs = Rien n'a été importé : déposez des fichiers de pack .cwtheme.
status-pack-detail-video-shader = avec une vidéo et un shader personnalisé
status-pack-detail-shader = avec un shader personnalisé
status-imported-pack-named = Pack « { $name } » importé ({ $detail }).
status-pack-renamed-with-detail = « { $name } » existait déjà : importé sous le nom « { $written } » ({ $detail }).
status-error-importing-pack = Erreur lors de l'importation du pack : { $error }
status-cancelled-nothing-imported = Annulé : rien n'a été importé.
status-applied-pack-with-video = Pack « { $name } » appliqué avec sa vidéo de fond.
status-applied-pack-video-missing = Pack « { $name } » appliqué : sa vidéo de fond est manquante ; réimportez le pack pour la restaurer.
status-applied-pack-theme-missing = Pack « { $name } » appliqué : son fichier de thème est manquant, des valeurs par défaut génériques ont donc été utilisées.
status-applied-pack = Pack « { $name } » appliqué.
status-error-applying-pack = Erreur lors de l'application du pack : { $error }

## Shared across multiple pages (src/modules/gui/view.rs)

common-active = Actif
common-active-now = Actif maintenant.
common-align-center = Centre
common-align-left = Gauche
common-align-right = Droite
common-apply = Appliquer
common-copied-to-clipboard = Copié dans le presse-papiers
common-copy = Copier
common-copy-to-clipboard = Copier dans le presse-papiers
common-create = Créer
common-hide = Masquer
common-open-folder = Ouvrir le dossier
common-recent-colours = Couleurs récentes
common-reset = Réinitialiser
common-retry = Réessayer
common-shape-circular = Circulaire
common-shape-linear = Linéaire
common-shape-square = Carrée
common-show = Afficher
common-start = Démarrer
common-stop = Arrêter

## Wallpaper page

wallpaper-mode-frosted-glass = Verre dépoli
wallpaper-mode-transparent = Transparent
wallpaper-mode-album-art = Pochette d'album
wallpaper-mode-album-colour = Couleur de l'album
wallpaper-mode-live-wallpaper = Fond d'écran animé
text-color-mode-automatic = Automatique
text-color-mode-custom = Personnalisé
wallpaper-preview-none = Aucun
wallpaper-preview-sample-title = Encore, et encore, et encore, et encore
wallpaper-preview-sample-caption = Je peux sentir l'adrénaline, je peux sentir le bruit
wallpaper-style-title = Style
wallpaper-frosted-glass-title = Verre dépoli
wallpaper-blur-amount = Intensité du flou
wallpaper-blur-amount-desc = À quel point le fond d'écran est flouté.
wallpaper-live-wallpaper-title = Fond d'écran animé
wallpaper-video-item = Vidéo
wallpaper-video-item-desc = Choisissez et gérez les vidéos depuis la page Fonds d'écran animés
wallpaper-text-title = Texte
wallpaper-text-colour = Couleur du texte
wallpaper-text-colour-desc = Le mode automatique choisit une couleur qui reste lisible sur votre fond d'écran.
wallpaper-page-title = Fond d'écran
wallpaper-page-summary = Les options du style sélectionné apparaissent ci-dessous.

## Live Wallpapers page

live-wallpapers-drop-release = Relâchez pour ajouter à votre bibliothèque
live-wallpapers-drop-prompt = Déposez ici des fichiers vidéo pour les ajouter (MP4, WebM)
live-wallpapers-library-title = Bibliothèque
live-wallpapers-no-videos = Aucune vidéo pour l'instant
live-wallpapers-no-videos-desc = Déposez des fichiers ci-dessus, ou utilisez Ouvrir le dossier pour les ajouter manuellement
live-wallpapers-playback-title = Lecture
live-wallpapers-prefer-canvas = Préférer le Canvas Spotify
live-wallpapers-prefer-canvas-desc = Si le titre en cours possède une boucle Canvas, l'afficher à la place du fond d'écran.
live-wallpapers-library-folder = Dossier de la bibliothèque
live-wallpapers-library-folder-desc = Les vidéos se trouvent dans ~/.config/cosmic-wallpaper/videos.
live-wallpapers-page-title = Fonds d'écran animés
live-wallpapers-page-summary = Vidéos en boucle utilisées comme arrière-plan. Cliquez sur une vignette pour l'activer.

## Themes page

theme-element-album-art = Pochette d'album
theme-element-track-info = Infos du titre
theme-element-lyrics = Paroles
theme-element-visualiser = Visualiseur
theme-element-weather = Météo
theme-element-effects = Effets
theme-align = Alignement
theme-position-x = Position X
theme-position-y = Position Y
theme-size = Taille
theme-text-size = Taille du texte
theme-shape = Forme
theme-docked = Ancrée
theme-docked-desc = Pendant la lecture de musique, la pochette se place dans le visualiseur circulaire et suit sa position et sa taille. Désactivez l'ancrage dans l'onglet Visualiseur pour utiliser ces réglages.
theme-band-order = Ordre des bandes
theme-rotation = Rotation
theme-amplitude = Amplitude
theme-bar-width = Largeur des barres
theme-cap-roundness = Arrondi des extrémités
theme-glow = Lueur
theme-reflection = Reflet
theme-led-segments = Segments LED
theme-peak-hold = Crêtes de pic
theme-peak-hold-desc = peak_hold - une petite crête lumineuse qui retient le pic récent de chaque barre puis retombe sous l'effet de la gravité
theme-dock-art = Ancrer la pochette dans l'anneau
theme-dock-art-desc = dock_art - la pochette suit la position et la taille de l'anneau
theme-lyric-bounce = Rebond des paroles
theme-spring-stiffness = Raideur du ressort
theme-spring-damping = Amortissement du ressort
theme-beat-pulse = Pulsation au rythme
theme-reset-section = Réinitialiser cette section
theme-reset-section-desc = Restaure { $element } à ses valeurs par défaut.
theme-page-theme-title = Thème
theme-editing = Modification
theme-editing-live-desc = Ce thème est actif : les modifications apparaissent sur votre bureau au fur et à mesure.
theme-editing-inactive-desc = Ce n'est pas le thème actif : les modifications sont enregistrées dans son fichier ; utilisez Appliquer pour les voir.
theme-manage-title = Gérer
theme-create-new = Créer un nouveau thème
theme-create-new-desc = Commence à partir d'un modèle complet et entièrement commenté.
theme-name-placeholder = Nom du thème
theme-name-empty-error = Le nom du thème ne peut pas être vide
theme-name-exists-error = Un thème portant ce nom existe déjà
theme-import = Importer
theme-import-desc = Déposez des fichiers de thème .toml n'importe où sur cette page pour les ajouter.
theme-page-title = Thèmes de mise en page
theme-page-summary = L'emplacement de chaque élément à l'écran. Faites glisser un réglage et regardez votre bureau suivre.

## Packs page

packs-your-packs-title = Vos packs
packs-none-yet = Aucun pack importé pour l'instant
packs-none-yet-desc = Déposez ci-dessous un fichier .cwtheme pour le voir apparaître ici.
packs-includes-video-active = Inclut une vidéo de fond - actif maintenant.
packs-includes-video = Inclut une vidéo de fond.
packs-layout-only = Mise en page uniquement.
packs-export-title = Exporter
packs-theme-to-bundle = Thème à regrouper
packs-export-desc-with-video = Regroupe la mise en page de ce thème, son shader personnalisé s'il en a un, et votre vidéo de fond actuellement active ({ $file }) - les vidéos ne sont pas liées à un thème en particulier, vérifiez donc que c'est bien celle que vous voulez partager.
packs-export-desc-no-video = Regroupe la mise en page de ce thème et son shader personnalisé, s'il en a un. Aucune vidéo de fond n'est active actuellement, le pack n'en inclura donc pas.
packs-export-pack = Exporter le pack
packs-folder = Dossier des packs
packs-folder-desc = Les fichiers .cwtheme exportés sont enregistrés dans ~/.config/cosmic-wallpaper/packs.
packs-import = Importer
packs-import-desc = Déposez un fichier .cwtheme n'importe où sur cette page pour l'ajouter.
packs-page-title = Packs
packs-page-summary = Partagez un look complet - mise en page, vidéo de fond et shader de visualiseur personnalisé - en un seul fichier.

## Now Playing page

now-playing-album-art-title = Pochette d'album
now-playing-show-album-art = Afficher la pochette d'album
now-playing-show-album-art-desc = La pochette actuelle, positionnée selon le thème de mise en page actif.
now-playing-lyrics-text-title = Paroles et texte
now-playing-show-lyrics = Afficher les paroles
now-playing-show-lyrics-desc = Paroles synchronisées du titre en cours, lorsqu'elles sont disponibles.
now-playing-font-family = Police
now-playing-page-title = Lecture en cours
now-playing-page-summary = Ce qui s'affiche pendant la lecture de musique : pochette, infos du titre et paroles.

## Visualiser page

visualiser-audio-response-title = Réponse audio
visualiser-bands = Bandes
visualiser-bands-desc = Le nombre de barres dessinées par le visualiseur.
visualiser-smoothing = Lissage
visualiser-smoothing-desc = Plus c'est élevé, plus c'est calme ; plus c'est bas, plus c'est réactif.
visualiser-page-title = Visualiseur
visualiser-page-summary = Des barres qui bougent avec ce qui est en train de jouer.

## Weather page

weather-unit-celsius = Celsius
weather-unit-fahrenheit = Fahrenheit
weather-poll-5min = 5 minutes
weather-poll-15min = 15 minutes
weather-poll-30min = 30 minutes
weather-poll-1hour = 1 heure
weather-page-title = Météo
weather-show-weather = Afficher la météo
weather-show-weather-desc = Conditions actuelles sur le bureau.
weather-hide-effects = Masquer les effets animés
weather-hide-effects-desc = Désactive les animations de pluie et de neige pour économiser de l'énergie.
weather-units = Unités
weather-location = Position
weather-location-desc = Latitude et longitude pour les prévisions. « Utiliser ma position » les estime à partir de votre adresse IP via ipapi.co.
weather-latitude-placeholder = Latitude
weather-longitude-placeholder = Longitude
weather-use-my-location = Utiliser ma position
weather-update-every = Actualiser toutes les
weather-page-summary = Conditions et effets superposés au fond d'écran.

## General page

general-checking-for-updates = Recherche de mises à jour...
general-up-to-date = À jour
general-check-for-updates = Rechercher des mises à jour
general-check-failed = Échec de la vérification : { $reason }
general-update-to = Mettre à jour vers { $tag }
general-release-page = Page de la version { $tag }
general-updating-to = Mise à jour vers { $tag }...
general-installed-restart = { $tag } installé - redémarrez l'application
general-engine-title = Moteur
general-wallpaper-engine = Moteur de fond d'écran
general-engine-running = En cours d'exécution (pid { $pid }).
general-engine-not-running = Arrêté.
general-start-on-login = Démarrer à la connexion
general-start-on-login-desc = Lance le moteur de fond d'écran à la connexion.
general-frame-rate-limit = Limite d'images par seconde
general-frame-rate-limit-desc = Une valeur plus basse économise l'énergie ; le moteur reste inactif quand rien n'est animé.
general-config-folder = Dossier de configuration
general-config-folder-desc = Toute la configuration du moteur se trouve ici.
general-about-title = À propos
general-version = Version
general-patch-notes = Notes de version
general-patch-notes-desc = Ce qui a changé dans la dernière version.
general-diagnostics = Diagnostic
general-diagnostics-desc = Version, extrait des journaux et infos GPU, prêts à coller dans un rapport.
general-something-broken = Un problème ?
general-something-broken-desc = Ouvre un rapport de bug pré-rempli avec les dernières erreurs jointes.
general-report-an-issue = Signaler un problème
general-setup-title = Configuration
general-not-in-launcher = Absent du lanceur d'applications
general-patch-notes-section-title = Notes de version
general-page-title = Général
general-page-summary = Comportement du moteur et maintenance.

general-language-title = Langue
general-language-desc = Remplace la langue du bureau pour cette application uniquement - utile pour une langue que votre bureau ne propose pas encore.
general-language-system-default = Langue du système
