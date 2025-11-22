# CrazyTrip Business Review Service

Microservicio para revisión manual de cuentas empresariales.

## Funcionalidad

- **Revisión Manual**: Staff puede revisar y aprobar/rechazar solicitudes de negocios
- **Dashboard de Revisión**: Ver todos los negocios pendientes de aprobación
- **Acciones**: Aprobar, Rechazar, Solicitar más información, Suspender
- **Estadísticas**: Métricas de revisiones para administradores
- **Audit Trail**: Registro de todas las acciones de revisión

## API Endpoints

### `GET /api/v1/health`
Health check del servicio

### `GET /api/v1/reviews/pending`
Lista de negocios pendientes de revisión

### `GET /api/v1/reviews/{business_id}`
Detalles de una revisión específica

### `POST /api/v1/reviews/{business_id}/action`
Realizar acción de revisión (aprobar/rechazar/etc)

**Body:**
```json
{
  "action": "approve|reject|request_more_info|suspend",
  "notes": "Optional reviewer notes",
  "rejection_reason": "Required if action is reject"
}
```

### `GET /api/v1/reviews/stats`
Estadísticas de revisiones

## Configuración

Crear archivo `.env`:

```env
HOST=127.0.0.1
PORT=8081
DATABASE_URL=postgresql://user:password@localhost/crazytrip_reviews
RUST_LOG=info
```

## Desarrollo

```bash
# Compilar
cargo build

# Ejecutar
cargo run

# Tests
cargo test
```

## Integración con crazytrip_server_users

Este servicio se comunica con `crazytrip_server_users` para:
1. Actualizar el estado de verificación de `BusinessAccount`
2. Crear entradas en `AuditLog` para compliance
3. Enviar notificaciones a los dueños de negocio

## Base de Datos

Tablas necesarias:
- `business_reviews`: Historial de revisiones
- `business_accounts`: Sincronizada con user service
- `audit_logs`: Logs de compliance (retention 1 año)

## Seguridad

- Autenticación requerida (JWT del user service)
- Solo usuarios con rol `Admin` o `Moderator` pueden acceder
- Todas las acciones son auditadas
- Rate limiting implementado

## TODO

- [ ] Implementar autenticación con JWT
- [ ] Conectar a base de datos PostgreSQL
- [ ] Implementar sincronización con user service
- [ ] Agregar sistema de notificaciones
- [ ] Implementar rate limiting
- [ ] Agregar tests unitarios e integración
- [ ] Documentar esquema de base de datos
