# Decisiones de UX/UI

Por qué la interfaz es como es. Cada decisión dice qué se hizo y la razón detrás.

## Generales

- **Tema oscuro único.** El personal clínico revisa imágenes y pasa muchas horas frente a la pantalla. Un fondo oscuro cansa menos la vista y deja que las imágenes y los colores de prioridad destaquen.
- **Interfaz compacta (texto base de 12px).** Se prioriza ver muchos casos y datos a la vez sin hacer scroll, como en otros sistemas clínicos.
- **Un solo color de acción.** El violeta `primary` marca solo la acción principal de cada pantalla. Así el usuario siempre sabe cuál es el siguiente paso.
- **El color nunca va solo.** Nivel y estado siempre se escriben (ALTA, En revisión). Así se entiende aunque la persona no distinga bien los colores o la pantalla esté en mala calidad.
- **La IA siempre marcada.** Todo resultado del proveedor simulado lleva «IA SIMULADA». No tiene valor clínico y nadie debe confundirlo con un diagnóstico.
- **Cada rol ve solo lo suyo.** El menú y las columnas cambian según el rol. El administrativo organiza sin ver datos clínicos; el administrador gestiona accesos sin ver pacientes.

## Por vista

- **Login.** Sin registro público: en un sistema clínico las cuentas las crea el administrador. Se entra con RUT porque así se identifica el personal en Chile.
- **Inicio.** Explica en una pantalla qué hace la plataforma y que la IA es simulada. El botón lleva directo a la cola, que es donde empieza el trabajo.
- **Cola de triage.** Es el centro del sistema: lo urgente siempre arriba (nivel, luego puntaje, luego antigüedad). La franja de color en cada fila permite leer la prioridad de un vistazo.
- **Casos.** Lista y detalle en la misma pantalla para no perder el contexto al revisar uno tras otro. El panel de revisión cambia según el estado del caso, así el médico solo ve lo que puede hacer en ese momento.
- **Nuevo caso + Paciente.** El paciente se crea en un modal, dentro del mismo flujo, para no sacar al usuario de lo que estaba haciendo. El consentimiento informado es obligatorio porque lo exige la Ley 19.628.
- **Imágenes.** La carga va separada de la revisión porque también la hace el administrativo. El análisis corre en segundo plano para que nadie se quede esperando.
- **Dashboard.** Solo cifras agregadas: sirve para evaluar el sistema y la IA, no para ver pacientes. Cada indicador muestra si cumple o no su meta.
- **Parámetros.** Las reglas del triage no están fijas en el código para que el equipo médico pueda ajustarlas. Cada cambio pide un motivo y crea una versión, para poder explicar por qué un caso tuvo cierto nivel.
- **Administración.** El administrador gestiona accesos pero no ve datos clínicos: los roles se separaron a propósito. La bitácora de auditoría existe para poder responder quién hizo qué y cuándo.
