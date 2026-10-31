"use strict";

const installButton = document.querySelector("#install");
const helpDialog = document.querySelector("#help-dialog");
const installHelp = document.querySelector("#install-help");
const pagesLink = document.querySelector("#pages-link");
const statusText = document.querySelector("#status");
const gameCanvas = document.querySelector("#glcanvas");
const displayMode = window.matchMedia("(display-mode: standalone)");
let installPrompt = null;
let gameReady = false;

function reportError(message, error) {
  console.error(message, error);
  statusText.textContent = message;
}

function isInstalled() {
  return displayMode.matches || navigator.standalone === true;
}

function updateInstallButton() {
  installButton.hidden = isInstalled();
}

function showHelp() {
  pagesLink.hidden = window.isSecureContext;
  if (!window.isSecureContext) {
    installHelp.textContent =
      "La instalacion y el modo sin conexion requieren HTTPS. " +
      "Con miniserve por HTTP en la IP del PC puedes jugar, pero no instalar. " +
      "Abre GitHub Pages para instalar en el telefono.";
  } else if (isInstalled()) {
    installHelp.textContent = "La aplicacion ya esta abierta en modo instalado.";
  } else if (/iPad|iPhone|iPod/.test(navigator.userAgent) ||
      (navigator.platform === "MacIntel" && navigator.maxTouchPoints > 1)) {
    installHelp.textContent =
      "En iPhone/iPad abre esta pagina en Safari, pulsa Compartir y " +
      "\"Anadir a pantalla de inicio\". iOS no ofrece un aviso automatico de instalacion.";
  } else if (installPrompt) {
    installHelp.textContent = "Pulsa Instalar para abrir el aviso de instalacion del navegador.";
  } else {
    installHelp.textContent =
      "Si el aviso aun no esta disponible, espera a que termine la carga o usa " +
      "\"Instalar aplicacion\" / \"Anadir a pantalla de inicio\" en el menu del navegador. " +
      "Los navegadores integrados en otras apps pueden requerir abrir la pagina en Chrome o Safari.";
  }
  releaseControls();
  helpDialog.showModal();
}

window.addEventListener("beforeinstallprompt", (event) => {
  event.preventDefault();
  installPrompt = event;
  updateInstallButton();
});

window.addEventListener("appinstalled", () => {
  installPrompt = null;
  installButton.hidden = true;
  statusText.textContent = "Aplicacion instalada.";
});

displayMode.addEventListener("change", updateInstallButton);
updateInstallButton();

installButton.addEventListener("click", async () => {
  if (!installPrompt) {
    showHelp();
    return;
  }
  const prompt = installPrompt;
  installPrompt = null;
  installButton.disabled = true;
  releaseControls();
  try {
    await prompt.prompt();
    const choice = await prompt.userChoice;
    statusText.textContent = choice.outcome === "accepted"
      ? "Instalacion aceptada; el navegador terminara de instalar."
      : "Instalacion cancelada. Puedes usar el menu del navegador para instalar mas tarde.";
  } catch (error) {
    reportError("No se pudo abrir la instalacion. Consulta Ayuda o el menu del navegador.", error);
  } finally {
    installButton.disabled = false;
    gameCanvas.focus();
  }
});

document.querySelector("#help").addEventListener("click", showHelp);
document.querySelector("#close-help").addEventListener("click", () => helpDialog.close());
helpDialog.addEventListener("close", () => {
  releaseControls();
  gameCanvas.focus();
});

const controlPointers = new Map();
const controlButtons = document.querySelectorAll("[data-code]");

function sendKey(type, code) {
  if (gameReady) {
    gameCanvas.dispatchEvent(new KeyboardEvent(type, { code, bubbles: true, cancelable: true }));
  }
}

function releasePointer(pointerId) {
  const button = controlPointers.get(pointerId);
  if (!button) return;
  controlPointers.delete(pointerId);
  if (![...controlPointers.values()].includes(button)) {
    button.classList.remove("pressed");
    sendKey("keyup", button.dataset.code);
  }
}

function releaseControls() {
  for (const pointerId of [...controlPointers.keys()]) releasePointer(pointerId);
}

for (const button of controlButtons) {
  button.addEventListener("pointerdown", (event) => {
    if (event.button !== 0 || !gameReady) return;
    event.preventDefault();
    const alreadyHeld = [...controlPointers.values()].includes(button);
    controlPointers.set(event.pointerId, button);
    button.setPointerCapture(event.pointerId);
    button.classList.add("pressed");
    if (!alreadyHeld) sendKey("keydown", button.dataset.code);
  });
  button.addEventListener("pointerup", (event) => releasePointer(event.pointerId));
  button.addEventListener("pointercancel", (event) => releasePointer(event.pointerId));
  button.addEventListener("lostpointercapture", (event) => releasePointer(event.pointerId));
  button.addEventListener("click", (event) => {
    if (event.detail === 0 && gameReady) {
      sendKey("keydown", button.dataset.code);
      requestAnimationFrame(() => sendKey("keyup", button.dataset.code));
    }
  });
}

window.addEventListener("blur", releaseControls);
document.addEventListener("visibilitychange", () => {
  if (document.hidden) releaseControls();
});
window.addEventListener("error", (event) => {
  reportError("Error al ejecutar el juego. Recarga la pagina y revisa la consola.", event.error);
});
window.addEventListener("unhandledrejection", (event) => {
  reportError("No se pudo cargar el juego. Comprueba la conexion y recarga.", event.reason);
});

async function enableOffline() {
  if (!window.isSecureContext || !("serviceWorker" in navigator)) {
    statusText.textContent = window.isSecureContext
      ? "Este navegador no admite el modo sin conexion."
      : "Modo HTTP: puedes jugar. Para instalar y jugar sin conexion, abre GitHub Pages (Ayuda).";
    return;
  }
  try {
    const registration = await navigator.serviceWorker.register("./sw.js", { scope: "./" });
    const worker = registration.installing;
    if (worker) {
      worker.addEventListener("statechange", () => {
        if (worker.state === "redundant") {
          reportError("No se pudo guardar la aplicacion sin conexion. Comprueba la conexion y recarga.",
            new Error("Service worker installation failed"));
        }
      });
    }
    await navigator.serviceWorker.ready;
    statusText.textContent = registration.waiting
      ? "Actualizacion lista. Cierra todas las ventanas del juego y vuelve a abrirlo."
      : "Listo para jugar sin conexion.";
    registration.addEventListener("updatefound", () => {
      const update = registration.installing;
      if (update) {
        update.addEventListener("statechange", () => {
          if (update.state === "installed" && navigator.serviceWorker.controller) {
            statusText.textContent = "Actualizacion lista. Cierra todas las ventanas del juego y vuelve a abrirlo.";
          }
        });
      }
    });
  } catch (error) {
    reportError("No se pudo activar el modo sin conexion. Comprueba la conexion y recarga.", error);
  }
}

// The loader supplied by the locked Macroquad dependency initializes its exports here.
miniquad_add_plugin({
  name: "platformer_web",
  version: 1,
  on_init() {
    gameReady = true;
    gameCanvas.focus();
    statusText.textContent = "Juego cargado. Preparando modo sin conexion...";
    enableOffline();
  },
});
load("./platformer.wasm");
