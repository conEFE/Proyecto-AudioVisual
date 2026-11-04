# Button

Botón de acción en dos variantes: principal (`primary`) para el siguiente paso y secundario (`secondary`) para lo demás.

Hecho a mano desde `src/components/ui.ts` (`primaryButton`, `secondaryButton`). Vista previa estática.

- **Principal:** fondo `primary`, texto `primary-foreground` en estilo `button`, `radius-sm`, padding `space-2` × `space-3`. Hover al 90%.
- **Secundario:** fondo `secondary`, texto `foreground`, peso 500. Hover al 80%.
- **Deshabilitado:** el principal pasa a `muted` con texto `muted-foreground`; el secundario baja a 50% de opacidad.
- Icono opcional de lucide a 14px antes del texto, con `space-1-5` de separación. Mientras carga, el texto se reemplaza por un spinner.

Úsalo así:
- Un solo botón principal por bloque.
- El texto es verbo + objeto: «Crear caso», «Subir imagen».
- Deshabilítalo mientras falte un dato obligatorio, y explica cerca qué falta.
