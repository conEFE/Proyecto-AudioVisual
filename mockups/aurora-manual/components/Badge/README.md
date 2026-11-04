# Badge

Etiqueta corta que muestra el nivel de triage, el estado del caso o la marca «IA SIMULADA».

Hecho a mano desde `src/components/Badges.tsx` y `src/constants.ts`. Vista previa estática.

- **Nivel (LevelBadge):** ALTA, MEDIA o BAJA en estilo `badge` (10px bold). Texto `level-*-text`, fondo `level-*-bg` y borde del mismo color al 30%. Sin nivel se escribe «Sin triage» en `muted-foreground`.
- **Estado (StatusBadge):** Abierto, Priorizado, En revisión o Cerrado, en 10px semibold con su `status-*-text` y fondo al 10%.
- **IA SIMULADA (SimulatedBadge):** `simulated-text` sobre `simulated-bg`, borde `simulated-border`, con un poco de espaciado entre letras. Lleva un `title` que explica que no es un resultado clínico real.
- Padding 2px × 8px, `radius-sm`, borde de 1px.

Úsalo así:
- Siempre con la palabra escrita: el color refuerza, no reemplaza.
- El badge «IA SIMULADA» acompaña a todo resultado de IA simulada, sin excepción.
