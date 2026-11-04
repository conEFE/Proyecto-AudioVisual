# QueueTable

Tabla de datos de la plataforma, en su forma más característica: la cola de triage con la franja de prioridad en cada fila.

Hecho a mano desde `src/components/TriageQueue.tsx`, `src/components/CaseManagement.tsx` y `src/components/ui.ts` (`thClass`, `tdClass`). Vista previa estática.

- Contenedor `card` con borde `border` y `radius-lg`, con scroll horizontal propio en pantallas chicas.
- Cabecera con fondo `secondary` al 50% y texto en estilo `overline` color `muted-foreground`.
- Celdas en `body`, padding `space-2` × `space-3`, sin saltos de línea; filas separadas por `border`.
- Código del caso en monoespaciada; nivel y estado con `Badge`.
- En la cola, cada fila lleva una franja izquierda de 4px del color de su nivel. Hover con `secondary` al 30%.
- Acción de la fila como enlace de texto al final («Abrir», «Ver»).

Úsalo así:
- El médico ve puntaje, regla y paciente; el administrativo, solo código, nivel, estado y espera. Las columnas cambian según el rol, no se ocultan con CSS.
- Estado vacío con una frase («No hay casos en la cola.»), nunca una tabla vacía.
