# Comedor · Gestión de Pensionistas (Escritorio)

Base de la aplicación de escritorio en **Rust + GPUI + GPUI Kit** orientada a **Windows 10/11** (Módulo 1).

## Estructura del Cargo Workspace

- `apps/desktop`: aplicación de escritorio con GPUI Kit (implementada en esta etapa).
- `apps/api`: reservado para el backend futuro (sin Axum ni WebSocket por ahora).
- `crates/domain`: reservado para modelos de dominio y reglas de negocio.
- `crates/shared`: reservado para tipos y contratos compartidos.
- `crates/biometric`: reservado para integración biométrica futura (sin SecuGen por ahora).

## Requisitos en Windows 10/11

- Rust `1.92` o posterior con toolchain MSVC (`x86_64-pc-windows-msvc`).
- Visual Studio 2022 Build Tools con la carga de trabajo **Desarrollo para el escritorio con C++** y **Windows SDK**.
- **CMake** disponible en el `PATH`.

## Comandos para compilar y ejecutar en Windows (PowerShell)

```powershell
# Verificar el toolchain activo
rustup default stable-x86_64-pc-windows-msvc

# Verificar todos los miembros del workspace
cargo check --workspace

# Ejecutar la aplicación de escritorio en modo desarrollo
cargo run -p desktop

# Compilar el ejecutable optimizado (release)
cargo build --release -p desktop
```

El binario compilado en release se genera en `target/release/desktop.exe`.

## Funcionalidades incluidas (Módulo 1)

- Ventana principal centrada (`1280×800`, tamaño mínimo `960×640`).
- Sidebar izquierdo colapsable usando `Sidebar`, `SidebarGroup`, `SidebarMenu` y `SidebarMenuItem` de GPUI Kit.
- Barra superior con botón de colapso, título/subtítulo dinámico de sección, estado de sesión, selector de tema y perfil de usuario (`Avatar`).
- Navegación funcional entre **Dashboard**, **Pensionistas**, **Identificación**, **Consumos**, **Caja**, **Reportes** y **Configuración**.
- Modo **claro** y **oscuro** integrado con el sistema de `Theme` de GPUI Kit (`Theme::change` + tokens globales).
- Manejo de errores tipado (`thiserror`) con alertas no bloqueantes (`Alert`) y verificaciones en segundo plano mediante `background_spawn`.

## Autenticación simulada (desarrollo)

Con `cargo run -p desktop` la aplicación inicia en la pantalla de acceso. En compilaciones de desarrollo se puede usar la contraseña ficticia `demo` con los usuarios `super_admin`, `admin`, `operator` o `cashier`. Cada uno dispone de permisos distintos. Para probar errores, usa con la misma contraseña `inactivo`, `expirado` o `error`. El menú de usuario permite cerrar sesión o simular su expiración.

La simulación no es un mecanismo de seguridad. En compilaciones `--release` está deshabilitada y el acceso permanece no disponible hasta integrar el servicio real. No introduzcas credenciales reales. No se guardan credenciales ni tokens. La autorización de producción deberá validarse en el servidor. El contrato futuro contempla `/api/v1/auth/login`, `/logout`, `/refresh` y `/me`; actualmente no se realizan solicitudes HTTP.

## Dashboard de demostración

El dashboard usa una fuente en memoria compartida (`apps/desktop/src/mock_data.rs`), sin conexión a una base de datos. Los módulos de pensionistas, consumos y caja muestran resúmenes de esa misma fuente; sus operaciones reales aún están pendientes. Al actualizar la colección compartida y notificar a la interfaz, las estadísticas se calculan nuevamente.

El selector permite navegar por día o volver a hoy. Desayunos y almuerzos son **números de consumos**, mientras que raciones cuenta las unidades entregadas. El importe consumido y los pagos corresponden al día seleccionado. La deuda es la suma de los saldos pendientes por pensionista hasta esa fecha; un saldo a favor no reduce la deuda de otra persona. Las cifras de pensionistas registrados y activos representan el estado actual, independientemente de la fecha. Los importes se calculan en céntimos.

Los datos se recrean al iniciar la aplicación y se pierden al cerrarla. Se muestran indicadores de carga, error y ausencia de actividad. Ejecuta `cargo test -p desktop` para comprobar los cálculos y permisos.

## Pensionistas de demostración

El módulo permite buscar por nombre, ID o celular, filtrar por estado, crear y editar datos, consultar el detalle y cambiar el estado con confirmación. Los identificadores se asignan automáticamente y no se editan. El celular es opcional y, si se indica, debe contener entre 7 y 15 dígitos. Las acciones respetan los permisos de la sesión.

El registro de huella es una simulación: cambia únicamente el estado en memoria y no captura ni almacena datos biométricos. Los movimientos, el saldo y las estadísticas se leen de la misma colección compartida. Todo se pierde al cerrar la aplicación; aún no hay persistencia ni conexión con dispositivos o servicios externos.

## Identificación biométrica simulada

En Identificación, usa **Conectar simulación**, luego **Modo espera** y una de las opciones de prueba. La coincidencia selecciona automáticamente el primer pensionista activo con código simulado; también se pueden probar resultados no reconocidos y errores. Cancelar invalida la operación en curso. La identificación no registra consumos: «Continuar» solo selecciona la persona para el módulo de Consumos, si la sesión tiene permisos.

Desde la ficha de un pensionista activo se puede simular el enrolamiento mediante un código de prueba único. No se captura ni almacena una huella real. Los códigos son datos temporales compartidos en memoria. `biometric_service.rs` define el contrato y los eventos internos para una futura integración; no utiliza DLL, USB ni WebSocket.
