# Field

Campo de formulario: etiqueta, control (input, select o textarea) y mensaje de error opcional.

Hecho a mano desde `src/components/ui.ts` (`inputClass`, `labelClass`, `errorBox`) y `src/components/PatientModal.tsx`. Vista previa estática.

- **Etiqueta:** estilo `label` en `foreground`, a 4px del control. Los obligatorios llevan « *».
- **Control:** fondo `background`, borde `input` de 1px, `radius-sm`, padding `space-2` × `space-3`, texto `body`. Con foco, anillo de 2px en `primary`.
- **Error de campo:** texto rojo de 10px debajo del control (por ejemplo, RUT inválido).
- **Error de formulario:** caja con `destructive` al 10% y texto `destructive-foreground`, arriba del formulario.

Úsalo así:
- El RUT se normaliza al salir del campo (12.345.678-5 → 12345678-5) y se valida con el dígito verificador.
- Pon el placeholder con un ejemplo de formato, no con la etiqueta.
- Nada de datos reales en ejemplos: usa RUT de prueba.
