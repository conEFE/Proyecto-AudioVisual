Plataforma web de apoyo a la detección temprana y priorización (triage) de casos sospechosos de cáncer de mama. Interfaz oscura, densa y sobria, pensada para personal clínico que trabaja muchas horas frente a la pantalla. La plataforma apoya, no reemplaza, el criterio médico, y la interfaz lo dice en cada lugar donde aparece un resultado de IA.

## Logo

- Usa el logo de `assets/Logos/logo-aurora.png` tal cual: ola de luz en degradé cian → índigo → violeta, un punto de luz rosa debajo y el nombre «PROYECTO AURORA» en blanco.
- El archivo trae su propio fondo `brand-night`. Colócalo sobre `brand-night` o sobre `background`, nunca sobre fondos claros ni fotos.
- No lo recrees, no le cambies los colores y no separes la ola del nombre.
- En la interfaz el logo grande solo aparece en Inicio (alto 224px). En el header se usa un cuadrado `primary` de 32px con el icono de pulso (Activity) en `primary-foreground`.

## Color

La interfaz tiene un solo tema, oscuro. Los colores vienen de las variables de shadcn/ui definidas en `src/index.css` y de la paleta de Tailwind para los estados.

- **Superficies:** `background` es el fondo de todo. Las tarjetas (`card`) usan el mismo color y se separan solo con un borde `border` de 1px, sin sombras.
- **Texto:** `foreground` para lo principal, `muted-foreground` para descripciones, fechas, etiquetas y ayudas.
- **Acción:** `primary` (violeta) marca la única acción principal de cada bloque y la pestaña activa del menú. El texto sobre `primary` va en `primary-foreground`. No uses `primary` para texto pequeño sobre `background`: da 3.48:1.
- **Secundario:** `secondary` para botones secundarios y para la cabecera de tablas (al 50%). `accent` solo como hover.
- **Errores:** caja con `destructive` al 10% de fondo y 20% de borde, texto `destructive-foreground`.
- **Niveles de triage:** ALTA `level-alta`, MEDIA `level-media`, BAJA `level-baja`. Se muestran siempre con la palabra escrita además del color (badge con el texto ALTA/MEDIA/BAJA), nunca solo con color.
- **Estados del caso:** Abierto `status-abierto-text`, Priorizado `status-priorizado-text`, En revisión `status-en-revision-text`, Cerrado `status-cerrado-text`, siempre con su nombre escrito.
- **IA simulada:** todo resultado del proveedor simulado lleva el badge «IA SIMULADA» en `simulated-text` sobre `simulated-bg`. Es obligatorio y no se oculta.
- **Marca:** los tokens `brand-*` salen del logo. Úsalos en piezas de marca (portadas, presentaciones, este manual), no en la interfaz de trabajo.

Contraste: `border` e `input` sobre `background` dan 1.34:1 y no cumplen el 3:1 recomendado para bordes de controles. Se documentan tal como están en el código.

## Tipografía

- Una sola familia: **Inter** (Google Fonts, pesos 300–700), con `system-ui` de respaldo.
- La interfaz es compacta: el tamaño base es `body` (12px). `caption` (10px) es el estilo más usado: fechas, ayudas, cabeceras.
- Títulos de sección en `page-title` (18px bold); títulos de tarjeta en `card-title` (14px semibold).
- Cabeceras de tabla en `overline`: 10px, mayúsculas, espaciado 0.05em, en `muted-foreground`.
- Códigos de caso (`AUR-2026-000151`) en fuente monoespaciada para que se lean carácter a carácter.
- Las cifras de tarjetas y KPI van en `kpi` (20px bold).

## Espaciado, bordes y layout

- Escala de 4px de Tailwind: `space-2` dentro de controles, `space-3` en celdas y grillas, `space-4` en tarjetas y entre secciones.
- Esquinas: `radius-sm` (4px) para controles y badges, `radius-lg` (8px) para tarjetas, tablas y modales.
- Sin sombras en la interfaz de trabajo. La única sombra es la del panel de notificaciones (`shadow-xl`), que flota sobre el contenido.
- Contenido centrado con ancho máximo `content-max-width` (1280px) y márgenes de 16px en móvil y `space-8` en escritorio.
- El header es sticky, con fondo `card` al 80% y desenfoque, y un menú de pestañas debajo que se desplaza en horizontal en pantallas chicas.
- Las vistas con lista y detalle (Casos) usan 5 columnas: 3 para la lista y 2 para el detalle.

## Iconografía

- Iconos de **lucide-react**, de trazo, en 14–16px (`w-3.5`/`w-4`), del color del texto que acompañan.
- Un icono acompaña al texto, no lo reemplaza. Los botones de solo icono (cerrar, notificaciones) llevan `aria-label`.
- No se usan emojis. La única excepción en el código es el lápiz «✎» que marca un nivel ajustado por el médico en la cola.

## Voz y textos

- Todo en español de Chile, trato de usted («Seleccione un caso», «Ingrese su RUT»).
- Botones con verbo y objeto: «Crear caso», «Tomar caso para revisión», «Registrar revisión y cerrar caso», «Guardar como nueva versión».
- Los avisos explican qué pasa y qué hacer: «Falta validar el resultado de IA de 1 imagen(es) (sección Imágenes) antes de cerrar.»
- Recordar el límite de la herramienta donde importa: «La plataforma apoya, no reemplaza, el criterio médico.»
- Datos de pacientes solo donde el rol lo permite. Las métricas se presentan como «agregadas, sin datos identificables de pacientes».
