# Mockup de vistas — Proyecto Aurora

Ingeniería inversa del frontend de [conEFE/Proyecto-Aurora](https://github.com/conEFE/Proyecto-Aurora)
(React + Tailwind) representada como mockup en un canvas de diseño.

**Canvas en línea:** https://claude.ai/artifact/QScm5PwiyA46PY6p4HeUFB
(privado hasta que se comparta desde el menú *Share* del canvas).

Los archivos `*.dc.html` y `canvas.json` de esta carpeta son la fuente del canvas (formato Design Component);
se versionan aquí como respaldo y no se abren directamente en el navegador.

## Inventario de vistas

| # | Artboard | Componente(s) origen | Rol mostrado |
|---|---|---|---|
| 01 | `Main.dc.html` — Login | `Login.tsx` | — |
| 02 | `Home.dc.html` — Inicio | `Home.tsx`, `Footer.tsx` | Médico |
| — | `Header.dc.html` — Header + navegación | `Header.tsx`, `NotificationBell.tsx` | Variable (tweak `role` / `active`) |
| 03 | `Cola.dc.html` — Cola de triage | `TriageQueue.tsx` | Médico |
| 04 | `Casos.dc.html` — Lista + detalle de caso | `CaseManagement.tsx`, `CaseDetail.tsx`, `SymptomsForm.tsx`, `ReviewPanel.tsx`, `TriageDetail.tsx`, `CaseImages.tsx` | Médico |
| 05 | `NuevoCaso.dc.html` — Nuevo caso + modal Paciente | `CaseManagement.tsx` (`NewCaseForm`), `PatientModal.tsx` | Administrativo |
| 06 | `Imagenes.dc.html` — Carga de imágenes | `ImageUpload.tsx`, `CaseImages.tsx` | Médico |
| 07 | `Dashboard.dc.html` — Métricas | `Dashboard.tsx` | Médico |
| 08 | `Parametros.dc.html` — Parámetros de triage | `TriageConfigPanel.tsx` | Médico |
| 09 | `AdminUsuarios.dc.html` — Usuarios | `AdminPanel.tsx` | Admin |
| 10 | `AdminAuditoria.dc.html` — Auditoría | `AdminPanel.tsx` (`AuditViewer`) | Admin |

## Navegación por rol (de `Header.tsx`)

- **MEDICO:** Inicio · Cola de triage · Casos · Imágenes · Dashboard · Parámetros (+ campana de casos ALTA)
- **ADMINISTRATIVO:** Inicio · Cola de triage · Casos · Imágenes · Dashboard
- **ADMIN:** Dashboard · Parámetros (solo lectura) · Administración — entra directo a Administración

Todos los datos (códigos, nombres, RUT, métricas, pesos) son ficticios o placeholders `[...]`.
