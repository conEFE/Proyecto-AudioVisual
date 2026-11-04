# StatCard

Tarjeta con una cifra grande y su etiqueta. Se usa para los conteos de la cola y los KPI del dashboard.

Hecho a mano desde `src/components/TriageQueue.tsx` (conteo por nivel) y `src/components/Dashboard.tsx` (`Kpi`). Vista previa estática.

- Fondo `card`, borde `border`, `radius-lg`, padding `space-3`.
- Cifra en estilo `kpi`; etiqueta en `caption` con color `muted-foreground`.
- **Variante nivel:** franja izquierda de 4px en `level-alta`, `level-media` o `level-baja`.
- **Variante KPI:** puede llevar una ayuda («Meta ≤ 5 min») y un indicador «Cumple» / «Fuera de meta», con icono y texto además del color.

Úsalo así:
- Una cifra por tarjeta, con su unidad («2.4 min», «420 ms»).
- Sin dato, muestra «—», no 0.
