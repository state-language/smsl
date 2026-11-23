#ifndef SMSL_H
#define SMSL_H
#define SMSL_ABI_VERSION 1
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif

typedef struct SmslError SmslError;
typedef struct SmslBuffer SmslBuffer;
typedef struct SmslBytes SmslBytes;
typedef struct SmslData SmslData;
typedef struct SmslDefinition SmslDefinition;
typedef struct SmslMachine SmslMachine;
typedef struct SmslEvent SmslEvent;
typedef struct SmslSnapshot SmslSnapshot;
typedef struct SmslStep SmslStep;
typedef struct SmslWorld SmslWorld;
typedef struct SmslExecutionSession SmslExecutionSession;
typedef struct SmslGraph SmslGraph;
typedef struct SmslSystemSession SmslSystemSession;
typedef struct SmslApplication SmslApplication;
typedef struct SmslLifecycleBinding SmslLifecycleBinding;
typedef struct SmslRemoteClient SmslRemoteClient;

typedef enum SmslStatus {
    SMSL_OK = 0, SMSL_INVALID_ARGUMENT = 1, SMSL_COMPILE_ERROR = 2,
    SMSL_NOT_FOUND = 3, SMSL_OUT_OF_RANGE = 4, SMSL_NO_TRANSITION = 5,
    SMSL_RUNTIME_ERROR = 6, SMSL_ENCODING_ERROR = 7
} SmslStatus;
enum { SMSL_FORMAT_TEXT = 0, SMSL_FORMAT_JSON = 1, SMSL_FORMAT_YAML = 2, SMSL_FORMAT_XML = 3, SMSL_FORMAT_TOML = 4 };
enum { SMSL_DATA_NULL = 0, SMSL_DATA_BOOL = 1, SMSL_DATA_STRING = 2, SMSL_DATA_U64 = 3, SMSL_DATA_ARRAY = 4, SMSL_DATA_OBJECT = 5 };
enum { SMSL_GRAPH_JSON = 0, SMSL_GRAPH_DOT = 1 };
enum { SMSL_EDGE_ANY = 0, SMSL_EDGE_OPERATION = 1, SMSL_EDGE_REACTION = 2 };
enum { SMSL_OUTGOING = 0, SMSL_INCOMING = 1, SMSL_SUCCESSORS = 2, SMSL_PREDECESSORS = 3, SMSL_REACHABLE = 4, SMSL_SHORTEST_PATH = 5 };

typedef struct SmslString { const uint8_t *data; size_t len; } SmslString;
typedef struct SmslBytesView { const uint8_t *data; size_t len; } SmslBytesView;
typedef struct SmslDiagnostic {
    SmslString code, message, path;
    size_t offset, line, column;
    uint8_t has_location, has_path;
} SmslDiagnostic;
typedef struct SmslContext { SmslString scope, subject, clock; } SmslContext;
typedef struct SmslEventRef { SmslString scope; uint64_t step; } SmslEventRef;
typedef struct SmslDataEntry { SmslString key; const SmslData *value; } SmslDataEntry;
typedef struct SmslNamePair { SmslString name, value; } SmslNamePair;

/* All non-null pointers must be valid for their use. Input spans are borrowed for
 * the call. Outputs own their handles; getters returning handles copy or retain.
 * Failed calls leave output unchanged and write one owned error when error != NULL.
 * Free errors and handles once; free(NULL) is allowed. Views last until owner free.
 * Immutable handles allow concurrent reads; callers synchronize World and frees. */
uint32_t smsl_abi_version(void);
void smsl_error_diagnostic(const SmslError *error, SmslDiagnostic *out);
void smsl_error_free(SmslError *error);
SmslString smsl_buffer_view(const SmslBuffer *buffer);
void smsl_buffer_free(SmslBuffer *buffer);

SmslStatus smsl_memory_compress(SmslString source, SmslBytes **out, SmslError **error);
SmslStatus smsl_memory_decompress(SmslBytesView source, SmslBuffer **out, SmslError **error);
SmslBytesView smsl_bytes_view(const SmslBytes *bytes);
void smsl_bytes_free(SmslBytes *bytes);

SmslData *smsl_data_unknown(void);
SmslData *smsl_data_null(void);
SmslData *smsl_data_u64(uint64_t value);
SmslStatus smsl_data_bool(uint8_t value, SmslData **out, SmslError **error);
SmslStatus smsl_data_string(SmslString value, SmslData **out, SmslError **error);
SmslStatus smsl_data_array(const SmslData *const *values, size_t count, SmslData **out, SmslError **error);
SmslStatus smsl_data_object(const SmslDataEntry *entries, size_t count, SmslData **out, SmslError **error);
SmslStatus smsl_data_load(SmslString source, uint32_t format, SmslData **out, SmslError **error);
SmslStatus smsl_data_dump(const SmslData *data, uint32_t format, SmslBuffer **out, SmslError **error);
uint32_t smsl_data_kind(const SmslData *data);
SmslStatus smsl_data_get_bool(const SmslData *data, uint8_t *out, SmslError **error);
SmslStatus smsl_data_get_u64(const SmslData *data, uint64_t *out, SmslError **error);
SmslStatus smsl_data_get_string(const SmslData *data, SmslString *out, SmslError **error);
SmslStatus smsl_data_count(const SmslData *data, size_t *out, SmslError **error);
SmslStatus smsl_data_at(const SmslData *data, size_t index, SmslData **out, SmslError **error);
SmslStatus smsl_data_key(const SmslData *data, size_t index, SmslString *out, SmslError **error);
SmslStatus smsl_data_get(const SmslData *data, SmslString key, SmslData **out, SmslError **error);
void smsl_data_free(SmslData *data);

/* Application requests load and validate portable definitions without creating
 * Worlds or providers. The returned data is independently owned. */
SmslStatus smsl_application_request(const SmslData *request, SmslData **out, SmslError **error);
/* Retained compiled applications share immutable models. Runtime requests borrow
 * the World; custom projection results include their captured basis. */
SmslStatus smsl_application_compile(const SmslData *request, SmslApplication **out, SmslError **error);
SmslStatus smsl_application_clone(const SmslApplication *application, SmslApplication **out, SmslError **error);
SmslStatus smsl_application_resolved(const SmslApplication *application, SmslData **out, SmslError **error);
SmslStatus smsl_application_world(const SmslApplication *application, SmslWorld **out, SmslError **error);
SmslStatus smsl_application_runtime_request(const SmslApplication *application, const SmslWorld *world, const SmslData *request, SmslData **out, SmslError **error);
/* A lifecycle binding owns its cursor and copies its validated declaration.
 * Apply atomically updates World and cursor; errors leave both unchanged.
 * Synchronize binding/World mutations; read pages from one source session. */
SmslStatus smsl_application_lifecycle_new(const SmslApplication *application, const SmslWorld *world, SmslString block, SmslLifecycleBinding **out, SmslError **error);
SmslStatus smsl_application_lifecycle_cursor(const SmslLifecycleBinding *binding, SmslData **out, SmslError **error);
SmslStatus smsl_application_lifecycle_apply(SmslLifecycleBinding *binding, SmslWorld *world, const SmslData *page, uint64_t time, SmslData **out, SmslError **error);
/* Applies the page to the execution-owned World in one transaction. Accepted
 * events enter execution history; failure preserves the session and cursor. */
SmslStatus smsl_application_lifecycle_apply_execution(SmslLifecycleBinding *binding, SmslExecutionSession *session, const SmslData *page, uint64_t time, SmslData **out, SmslError **error);
void smsl_application_lifecycle_free(SmslLifecycleBinding *binding);
SmslStatus smsl_world_runtime_request(SmslWorld *world, const SmslData *request, SmslData **out, SmslError **error);
SmslStatus smsl_snapshot_profile_request(const SmslSnapshot *snapshot, const SmslData *request, SmslData **out, SmslError **error);
void smsl_application_free(SmslApplication *application);
/* Remote protocol state performs no I/O. Hosts execute returned HTTP commands
 * and supply monotonically increasing millisecond timestamps. */
SmslStatus smsl_remote_client_new(const SmslData *config, SmslRemoteClient **out, SmslError **error);
SmslStatus smsl_remote_client_request(SmslRemoteClient *client, const SmslData *request, SmslData **out, SmslError **error);
void smsl_remote_client_free(SmslRemoteClient *client);

/* Execution owns an independent copy of its initial World. World access returns a copy.
 * Definition requests: decode(source,format), encode(definition,format), validate(definition).
 * Session requests: apply(input), inspect, save(format), world(format).
 * Planning requests: search(goal,operators?,limits), enabled(goal,operators?),
 * validate(goal,operators?,plan). For goal-free finite exploration, use
 * analyze(operators?) or explore(operators?,max_states?). Every request is
 * tagged by its operation field. */
SmslStatus smsl_execution_request(const SmslData *request, SmslData **out, SmslError **error);
SmslStatus smsl_execution_session_new(const SmslData *definition, const SmslWorld *world, SmslExecutionSession **out, SmslError **error);
SmslStatus smsl_execution_session_restore(SmslString source, uint32_t format, SmslExecutionSession **out, SmslError **error);
SmslStatus smsl_execution_session_request(SmslExecutionSession *session, const SmslData *request, SmslData **out, SmslError **error);
SmslStatus smsl_execution_session_world(const SmslExecutionSession *session, SmslWorld **out, SmslError **error);
void smsl_execution_session_free(SmslExecutionSession *session);
SmslStatus smsl_planning_request(const SmslMachine *machine, const SmslSnapshot *state, const SmslData *request, SmslData **out, SmslError **error);

/* System requests validate descriptions without running providers. Sessions are
 * mutable; synchronize their calls and destruction. Providers stay host-owned. */
SmslStatus smsl_system_request(const SmslData *request, SmslData **out, SmslError **error);
SmslStatus smsl_system_session_new(const SmslData *definition, const SmslData *basis, SmslSystemSession **out, SmslError **error);
/* Driven sessions retain provider lifecycle state and emit host callback commands.
 * Provider descriptions are portable catalog records; callbacks stay with the host. */
SmslStatus smsl_system_session_driver_new(const SmslData *definition, const SmslData *basis, const SmslData *providers, SmslSystemSession **out, SmslError **error);
SmslStatus smsl_system_session_drive(SmslSystemSession *session, const SmslData *request, SmslData **out, SmslError **error);
SmslStatus smsl_system_session_request(SmslSystemSession *session, const SmslData *request, SmslData **out, SmslError **error);
void smsl_system_session_free(SmslSystemSession *session);

SmslStatus smsl_definition_from_data(const SmslData *data, SmslDefinition **out, SmslError **error);
SmslStatus smsl_definition_load(SmslString source, uint32_t format, SmslDefinition **out, SmslError **error);
SmslStatus smsl_definition_data(const SmslDefinition *definition, SmslData **out, SmslError **error);
SmslStatus smsl_definition_dump(const SmslDefinition *definition, uint32_t format, SmslBuffer **out, SmslError **error);
SmslStatus smsl_definition_namespace(const SmslDefinition *definition, SmslString prefix, uint8_t events, SmslDefinition **out, SmslError **error);
SmslStatus smsl_definition_map_events(const SmslDefinition *definition, const SmslNamePair *mapping, size_t count, SmslDefinition **out, SmslError **error);
SmslStatus smsl_definition_compose(const SmslDefinition *const *definitions, size_t count, SmslDefinition **out, SmslError **error);
void smsl_definition_free(SmslDefinition *definition);

SmslStatus smsl_module_decode(SmslString source, uint32_t format, SmslData **out, SmslError **error);
SmslStatus smsl_module_encode(const SmslData *module, uint32_t format, SmslBuffer **out, SmslError **error);
SmslStatus smsl_machine_link(SmslString root, const SmslData *sources, SmslMachine **out, SmslError **error);
SmslStatus smsl_machine_load(SmslString source, uint32_t format, SmslMachine **out, SmslError **error);
SmslStatus smsl_machine_compile(const SmslDefinition *definition, SmslMachine **out, SmslError **error);
SmslStatus smsl_machine_definition(const SmslMachine *machine, SmslDefinition **out, SmslError **error);
SmslStatus smsl_machine_dump(const SmslMachine *machine, uint32_t format, SmslBuffer **out, SmslError **error);
SmslStatus smsl_machine_successor(const SmslMachine *machine, SmslString branch, SmslString state, SmslString operation, SmslData **out, SmslError **error);
SmslStatus smsl_machine_binding_value(const SmslMachine *machine, SmslString branch, SmslString fact, SmslString state, uint8_t *out, SmslError **error);
SmslStatus smsl_machine_start(const SmslMachine *machine, const SmslContext *context, SmslSnapshot **out, SmslError **error);
SmslStatus smsl_machine_restore(const SmslMachine *machine, SmslString source, uint32_t format, SmslSnapshot **out, SmslError **error);
SmslStatus smsl_machine_graph(const SmslMachine *machine, SmslGraph **out, SmslError **error);
void smsl_machine_free(SmslMachine *machine);

SmslStatus smsl_event_new(SmslString name, uint64_t time, const SmslContext *context, const SmslEventRef *cause, const SmslData *payload, SmslEvent **out, SmslError **error);
SmslStatus smsl_event_from_data(const SmslData *data, SmslEvent **out, SmslError **error);
SmslStatus smsl_event_load(SmslString source, uint32_t format, SmslEvent **out, SmslError **error);
SmslStatus smsl_event_data(const SmslEvent *event, SmslData **out, SmslError **error);
SmslStatus smsl_event_dump(const SmslEvent *event, uint32_t format, SmslBuffer **out, SmslError **error);
void smsl_event_free(SmslEvent *event);

SmslStatus smsl_snapshot_data(const SmslSnapshot *snapshot, SmslData **out, SmslError **error);
SmslStatus smsl_snapshot_branch(const SmslSnapshot *snapshot, SmslString branch, SmslData **out, SmslError **error);
SmslStatus smsl_snapshot_advance(const SmslSnapshot *snapshot, const SmslEvent *event, SmslStep **out, SmslError **error);
SmslStatus smsl_snapshot_confirm(const SmslSnapshot *snapshot, const SmslEvent *event, const SmslNamePair *operations, size_t count, SmslStep **out, SmslError **error);
SmslStatus smsl_snapshot_reset(const SmslSnapshot *snapshot, const SmslContext *context, SmslSnapshot **out, SmslError **error);
SmslStatus smsl_snapshot_dump(const SmslSnapshot *snapshot, uint32_t format, SmslBuffer **out, SmslError **error);
SmslStatus smsl_snapshot_activation(const SmslSnapshot *snapshot, SmslString branch, SmslData **out, SmslError **error);
SmslStatus smsl_snapshot_applicable(const SmslSnapshot *snapshot, SmslString branch, SmslString action, const SmslData *arguments, uint8_t *out, SmslError **error);
SmslStatus smsl_snapshot_evolve(const SmslSnapshot *snapshot, const SmslData *change, SmslSnapshot **out, SmslError **error);
SmslStatus smsl_snapshot_abstraction(const SmslSnapshot *snapshot, SmslString branch, SmslString name, SmslData **out, SmslError **error);
SmslStatus smsl_snapshot_concretize(const SmslSnapshot *snapshot, SmslString branch, SmslString name, const SmslData *requirements, uint8_t knowledge, SmslData **out, SmslError **error);
SmslStatus smsl_snapshot_validate_concretization(const SmslSnapshot *snapshot, const SmslData *query, SmslError **error);
SmslStatus smsl_snapshot_matches_concretization(const SmslSnapshot *snapshot, const SmslData *query, const SmslSnapshot *candidate, SmslData **out, SmslError **error);
/* kind: 0 abstraction value, 1 concretization. Validates and copies the record. */
SmslStatus smsl_abstraction_record(const SmslData *record, uint32_t kind, SmslData **out, SmslError **error);
SmslStatus smsl_snapshot_definition(const SmslSnapshot *snapshot, SmslDefinition **out, SmslError **error);
void smsl_snapshot_free(SmslSnapshot *snapshot);
SmslStatus smsl_step_snapshot(const SmslStep *step, SmslSnapshot **out, SmslError **error);
SmslStatus smsl_step_data(const SmslStep *step, SmslData **out, SmslError **error);
SmslStatus smsl_step_event(const SmslStep *step, size_t index, SmslEvent **out, SmslError **error);
void smsl_step_free(SmslStep *step);

SmslWorld *smsl_world_new(void);
SmslStatus smsl_world_spawn(SmslWorld *world, SmslString name, const SmslMachine *machine, const SmslContext *context, SmslSnapshot **out, SmslError **error);
SmslStatus smsl_world_remove(SmslWorld *world, SmslString name, SmslError **error);
SmslStatus smsl_world_snapshot(const SmslWorld *world, SmslString name, SmslSnapshot **out, SmslError **error);
SmslStatus smsl_world_advance(SmslWorld *world, SmslString name, const SmslEvent *event, SmslStep **out, SmslError **error);
SmslStatus smsl_world_confirm(SmslWorld *world, SmslString name, const SmslEvent *event, const SmslNamePair *operations, size_t count, SmslStep **out, SmslError **error);
SmslStatus smsl_world_reset(SmslWorld *world, SmslString name, const SmslContext *context, SmslSnapshot **out, SmslError **error);
SmslStatus smsl_world_resolve(const SmslWorld *world, const SmslEventRef *reference, SmslEvent **out, SmslError **error);
SmslStatus smsl_world_instances(const SmslWorld *world, SmslData **out, SmslError **error);
SmslStatus smsl_world_record_count(const SmslWorld *world, size_t *out, SmslError **error);
SmslStatus smsl_world_dump(const SmslWorld *world, uint32_t format, SmslBuffer **out, SmslError **error);
SmslStatus smsl_world_restore(SmslString source, uint32_t format, SmslWorld **out, SmslError **error);
SmslStatus smsl_world_request(SmslWorld *world, const SmslData *request, SmslData **out, SmslError **error);
void smsl_world_free(SmslWorld *world);

SmslStatus smsl_graph_data(const SmslGraph *graph, SmslData **out, SmslError **error);
SmslStatus smsl_graph_node(const SmslGraph *graph, SmslString branch, SmslString state, size_t *out, SmslError **error);
SmslStatus smsl_graph_traverse(const SmslGraph *graph, uint32_t operation, size_t source, size_t target, uint32_t kind, SmslData **out, SmslError **error);
SmslStatus smsl_graph_dump(const SmslGraph *graph, uint32_t format, SmslBuffer **out, SmslError **error);
void smsl_graph_free(SmslGraph *graph);

enum { SMSL_TYPE_BOOL = 0, SMSL_TYPE_STRING = 1, SMSL_TYPE_TIME = 2, SMSL_TYPE_EVENT = 3, SMSL_TYPE_UINT = 4 };
enum { SMSL_EXPR_LITERAL = 0, SMSL_EXPR_READ = 1, SMSL_EXPR_NOT = 2, SMSL_EXPR_AND = 3, SMSL_EXPR_OR = 4, SMSL_EXPR_EQ = 5, SMSL_EXPR_NE = 6, SMSL_EXPR_LT = 7, SMSL_EXPR_LE = 8, SMSL_EXPR_GT = 9, SMSL_EXPR_GE = 10, SMSL_EXPR_IS_UNKNOWN = 11, SMSL_EXPR_ADD = 12, SMSL_EXPR_SUB = 13, SMSL_EXPR_MUL = 14, SMSL_EXPR_DIV = 15, SMSL_EXPR_REM = 16 };
typedef struct SmslExprRecord {
    uint32_t kind;
    const SmslData *literal;
    SmslString source, field;
    const struct SmslExprRecord *left, *right;
} SmslExprRecord;
typedef struct SmslFactRecord { SmslString name; const SmslData *const *values; size_t value_count; const SmslData *initial; const SmslExprRecord *derived; } SmslFactRecord;
typedef struct SmslAssignmentRecord { SmslString fact; const SmslData *value; } SmslAssignmentRecord;
typedef struct SmslTransitionRecord { SmslString operation, target; const struct SmslUpdateRecord *updates; size_t update_count; } SmslTransitionRecord;
typedef struct SmslBindingRecord { SmslString branch, fact; } SmslBindingRecord;
typedef struct SmslReadRecord { SmslString alias, branch; } SmslReadRecord;
typedef struct SmslFieldRecord { SmslString name; uint32_t type; const SmslData *initial; } SmslFieldRecord;
typedef struct SmslUpdateRecord { SmslString field; const SmslExprRecord *value; } SmslUpdateRecord;
typedef struct SmslReactionRecord {
    SmslString event;
    const SmslString *target;
    const SmslExprRecord *guard;
    const SmslUpdateRecord *updates; size_t update_count;
    const SmslString *emit; size_t emit_count;
} SmslReactionRecord;
typedef struct SmslStateRecord {
    SmslString name;
    const SmslAssignmentRecord *values; size_t value_count;
    const SmslTransitionRecord *transitions; size_t transition_count;
    const SmslReactionRecord *reactions; size_t reaction_count;
} SmslStateRecord;
typedef struct SmslBranchRecord {
    SmslString name;
    const SmslString *initial;
    const SmslExprRecord *activating;
    const SmslFactRecord *facts; size_t fact_count;
    const SmslStateRecord *states; size_t state_count;
    const SmslBindingRecord *bindings; size_t binding_count;
    const SmslFieldRecord *data; size_t data_count;
    const SmslReadRecord *reads; size_t read_count;
    uint8_t is_mutable;
    const SmslReactionRecord *reactions; size_t reaction_count;
    const struct SmslPredicateRecord *labels; size_t label_count;
    const struct SmslActionRecord *actions; size_t action_count;
    const struct SmslAbstractionRecord *abstractions; size_t abstraction_count;
} SmslBranchRecord;
typedef struct SmslAbstractionRecord {
    SmslString name;
    const SmslString *facts; size_t fact_count;
} SmslAbstractionRecord;
typedef struct SmslPredicateRecord { SmslString name; const SmslExprRecord *expression; } SmslPredicateRecord;
typedef struct SmslActionRecord {
    SmslString name;
    const struct SmslEventFieldRecord *parameters; size_t parameter_count;
    const SmslExprRecord *precondition;
    const SmslString *outcomes; size_t outcome_count;
} SmslActionRecord;
typedef struct SmslEventFieldRecord { SmslString name; uint32_t type; uint8_t nullable; } SmslEventFieldRecord;
typedef struct SmslEventRecord { SmslString name; const SmslEventFieldRecord *fields; size_t field_count; } SmslEventRecord;
typedef struct SmslDefinitionRecord {
    SmslString version;
    const SmslEventRecord *events; size_t event_count;
    const SmslBranchRecord *branches; size_t branch_count;
} SmslDefinitionRecord;
SmslStatus smsl_definition_new(const SmslDefinitionRecord *record, SmslDefinition **out, SmslError **error);

#ifdef __cplusplus
}
#endif
#include "smsl-providers.h"
#endif
