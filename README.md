# Plataformer

Juego de plataformas en Rust con Macroquad. La version web es una PWA con
controles tactiles, instalacion y recursos locales para jugar sin conexion.

## Escritorio

```powershell
cargo run --release
```

## Web con miniserve en el puerto 8091

Preparacion (una sola vez):

```powershell
rustup target add wasm32-unknown-unknown
cargo install miniserve
```

Desde la raiz del repositorio:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build-web.ps1
miniserve .\dist --index index.html --port 8091 --interfaces 0.0.0.0
```

Abre `http://localhost:8091` en el PC o `http://IP-DEL-PC:8091` en un telefono
conectado a la misma red. Si Windows pide permiso, permite el servidor solo
en redes privadas. Sirve **dist**, no la raiz del proyecto ni el directorio
de compilacion.

El navegador permite service workers en `localhost`, pero **no** en la IP
del PC por HTTP. En el telefono el juego funciona por HTTP; para instalar
y usar el modo sin conexion, abre la version HTTPS de
[GitHub Pages](https://rodrigopanto-create.github.io/plataformer/).
El boton **Instalar** explica esta limitacion y ofrece ese enlace.
Para instalar desde miniserve en el telefono seria necesario configurar
HTTPS con un certificado que el telefono considere confiable.

## Instalacion en el telefono

- Android/Chrome: pulsa **Instalar** y acepta el aviso. Si el navegador no
  ofrece el aviso, usa **Instalar aplicacion** / **Anadir a pantalla de
  inicio** en su menu.
- iPhone/iPad: abre la URL HTTPS en Safari y usa **Compartir > Anadir a
  pantalla de inicio**. El boton muestra estas instrucciones porque iOS
  no implementa el aviso de instalacion de Chrome.
- Espera el mensaje **Listo para jugar sin conexion** antes de desconectar.
  Para aplicar una actualizacion, cierra todas las ventanas de la
  aplicacion y vuelve a abrirla. Las versiones del cache dependen del
  contenido de la compilacion.

## GitHub Pages

En **Settings > Pages > Source** selecciona **GitHub Actions**.
El workflow `pages.yml` compila WASM y despliega `dist` al hacer push a
`master`; tambien se puede ejecutar manualmente desde **Actions**.
El mismo script genera el sitio local y el de Pages. El cargador JavaScript
se copia de la version de Macroquad fijada en `Cargo.lock`, con su licencia,
sin depender de un CDN al jugar.
El script corrige una declaracion de variable ausente en el plugin
`quad_net` del cargador de Macroquad 0.4.16 para evitar un error de JavaScript
en modo estricto.

## Controles

- A/D o flechas: mover.
- Espacio, W o arriba: saltar.
- F/X: fuego.
- Enter: inicio/continuar.
- R: reiniciar. Escape: menu.
- Telefono: controles inferiores con soporte de pulsaciones simultaneas;
  reiniciar y menu estan disponibles en **Ayuda**.
