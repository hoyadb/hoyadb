const kinds=new Set(['BackendFailure','BackupPolicyFailure','MigrationFailure','SchemaConflict','InvalidSchema','InvalidQuery','InvalidRow','TableNotFound','RowAlreadyExists','RowNotFound','InvalidCommand','InvalidTicket','Busy','Capacity','InvalidPayload','Protocol','UnknownLocalReference','RollbackConflict','Closed','LimitExceeded','AuthorityRequired','StalePull','PullConflict','InvalidRequest','InvalidHandle','UnexpectedPending','UnsupportedOperation','Cancelled']);
export const failure=kind=>{kind=kinds.has(kind)?kind:'BackendFailure';return Object.assign(new Error(kind),{kind});};
export function check(value,kind='InvalidRequest'){if(!value)throw failure(kind);}
export function publicFailure(error){let kind;try{kind=error?.kind;}catch{}return failure(kind);}
