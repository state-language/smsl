#ifndef SMSL_PROVIDERS_H
#define SMSL_PROVIDERS_H
#include "smsl.h"
#ifdef __cplusplus
extern "C" {
#endif

typedef struct SmslProviderRegistry SmslProviderRegistry;
typedef struct SmslSystemRuntime SmslSystemRuntime;

/* Configuration and invocation data are borrowed during callbacks. On success,
 * create transfers one session to destroy, invoke transfers one owned SmslData,
 * and cancel writes 1 only after stopping is acknowledged. On failure callbacks
 * transfer one owned SmslError and leave outputs unchanged. Callback functions
 * must outlive registry and runtimes. Factory context is borrowed: keep it alive
 * until every derived runtime is freed, including runtimes that have not invoked
 * a provider yet. Start validates bindings; create runs lazily on first use.
 * A runtime may outlive its registry. Synchronize access.
 * Destroy releases resources, must not throw, and cannot report a failure.
 * Callbacks must not reenter their registry or runtime. */
typedef SmslStatus (*SmslProviderCreate)(void *context, const SmslData *config, void **session, SmslError **error);
typedef SmslStatus (*SmslProviderInvoke)(void *session, const SmslData *invocation, SmslData **outputs, SmslError **error);
typedef SmslStatus (*SmslProviderCancel)(void *session, SmslString request_id, uint8_t *acknowledged, SmslError **error);
typedef void (*SmslProviderDestroy)(void *session);
typedef struct SmslProviderCallbacks {
    SmslProviderCreate create;
    SmslProviderInvoke invoke;
    SmslProviderCancel cancel;
    SmslProviderDestroy destroy;
} SmslProviderCallbacks;

SmslStatus smsl_error_new(SmslString code, SmslString message, SmslError **out, SmslError **error);
SmslProviderRegistry *smsl_provider_registry_new(void);
SmslStatus smsl_provider_registry_register(SmslProviderRegistry *registry, SmslString id,
    const SmslString *contracts, size_t contract_count, void *context,
    const SmslProviderCallbacks *callbacks, uint8_t *out, SmslError **error);
SmslStatus smsl_provider_registry_start(const SmslProviderRegistry *registry,
    const SmslData *definition, const SmslData *basis, SmslSystemRuntime **out, SmslError **error);
SmslStatus smsl_system_runtime_invoke(SmslSystemRuntime *runtime, SmslString block,
    SmslString request_id, const SmslData *inputs, uint8_t allow_effects, SmslData **out, SmslError **error);
SmslStatus smsl_system_runtime_request(SmslSystemRuntime *runtime, const SmslData *request,
    SmslData **out, SmslError **error);
SmslStatus smsl_system_runtime_cancel(SmslSystemRuntime *runtime, SmslString block,
    SmslString request_id, uint8_t *out, SmslError **error);
SmslStatus smsl_system_runtime_close(SmslSystemRuntime *runtime, uint8_t *out, SmslError **error);
/* Resolve pending work before freeing a runtime; destruction does not stop a device. */
void smsl_system_runtime_free(SmslSystemRuntime *runtime);
void smsl_provider_registry_free(SmslProviderRegistry *registry);

#ifdef __cplusplus
}
#endif
#endif
