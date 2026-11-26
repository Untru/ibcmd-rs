import { RpcError } from "./rpc";
export class ImportValidationError extends Error {}

// Server chains may contain credentials, user names and absolute paths.
export function importErrorMessage(error: unknown): string {
  if (error instanceof RpcError) {
    if (error.code === -32016)
      return "Database changes were published; backup update failed. Inspect recovery before retrying";
    return error.message;
  }
  if (error instanceof ImportValidationError) return error.message;
  return "Import failed";
}
export interface ImportRpc {
  request(
    method: string,
    params: object,
    signal?: AbortSignal,
    timeout?: number,
  ): Promise<any>;
}
export interface ImportRequest {
  infobase: string;
  sourceDir: string;
  files?: string[];
  objects?: string[];
  changed?: boolean;
}
export interface ImportUi {
  current(): Promise<boolean>;
  confirm(message: string): Promise<boolean>;
}
// Mirrors metadata_model::index::ROOT_COLLECTIONS; the parity regression guards drift.
const kinds: Record<string, string> = {
  Languages: "Language",
  Subsystems: "Subsystem",
  StyleItems: "StyleItem",
  Styles: "Style",
  CommonPictures: "CommonPicture",
  SessionParameters: "SessionParameter",
  Roles: "Role",
  CommonTemplates: "CommonTemplate",
  FilterCriteria: "FilterCriterion",
  CommonModules: "CommonModule",
  CommonAttributes: "CommonAttribute",
  ExchangePlans: "ExchangePlan",
  XDTOPackages: "XDTOPackage",
  WebServices: "WebService",
  HTTPServices: "HTTPService",
  WSReferences: "WSReference",
  WebSocketClients: "WebSocketClient",
  EventSubscriptions: "EventSubscription",
  ScheduledJobs: "ScheduledJob",
  SettingsStorages: "SettingsStorage",
  FunctionalOptions: "FunctionalOption",
  FunctionalOptionsParameters: "FunctionalOptionsParameter",
  DefinedTypes: "DefinedType",
  CommonCommands: "CommonCommand",
  CommandGroups: "CommandGroup",
  Constants: "Constant",
  CommonForms: "CommonForm",
  Catalogs: "Catalog",
  Documents: "Document",
  DocumentNumerators: "DocumentNumerator",
  Sequences: "Sequence",
  DocumentJournals: "DocumentJournal",
  Enums: "Enum",
  Reports: "Report",
  DataProcessors: "DataProcessor",
  InformationRegisters: "InformationRegister",
  AccumulationRegisters: "AccumulationRegister",
  ChartsOfCharacteristicTypes: "ChartOfCharacteristicTypes",
  ChartsOfAccounts: "ChartOfAccounts",
  AccountingRegisters: "AccountingRegister",
  ChartsOfCalculationTypes: "ChartOfCalculationTypes",
  CalculationRegisters: "CalculationRegister",
  BusinessProcesses: "BusinessProcess",
  Tasks: "Task",
  IntegrationServices: "IntegrationService",
  Bots: "Bot",
  ExternalDataSources: "ExternalDataSource",
};
function owner(path: string): string {
  const [collection, file] = path.replace(/\\/g, "/").split("/");
  if (!kinds[collection] || !file)
    throw new ImportValidationError(`Cannot add dependencies for ${path}`);
  return `${kinds[collection]}.${file.replace(/\.xml$/, "")}`;
}
async function check(ui: ImportUi, signal?: AbortSignal) {
  if (signal?.aborted)
    throw new ImportValidationError("Import cancelled before publication");
  if (!(await ui.current()))
    throw new ImportValidationError(
      "Connection or settings changed; prepare the import again",
    );
}
export async function runImport(
  rpc: ImportRpc,
  request: ImportRequest,
  ui: ImportUi,
  signal?: AbortSignal,
): Promise<any | undefined> {
  await check(ui, signal);
  let prepared = await rpc.request(
    "objects/import/prepare",
    request,
    signal,
    600000,
  );
  await check(ui, signal);
  const missing = prepared.preflight.missingReferences as {
    reference: string;
    suggestedObject?: string;
  }[];
  if (missing.length) {
    const suggestions = [
      ...new Set(
        missing.map((m) => m.suggestedObject).filter((v): v is string => !!v),
      ),
    ];
    const refs = missing.map((m) => m.reference).join(", ");
    if (!suggestions.length)
      throw new ImportValidationError(`Unresolved references: ${refs}`);
    if (
      !(await ui.confirm(
        `Unresolved references: ${refs}. Add ${suggestions.join(", ")} and recheck? Selected files will include their complete objects. New objects may be unsupported.`,
      ))
    )
      return;
    await check(ui, signal);
    request = {
      ...request,
      files: undefined,
      changed: false,
      objects: [
        ...new Set([
          ...(request.objects ?? []),
          ...(
            request.files ?? (request.changed ? prepared.preflight.files : [])
          ).map(owner),
          ...suggestions,
        ]),
      ],
    };
    prepared = await rpc.request(
      "objects/import/prepare",
      request,
      signal,
      600000,
    );
    await check(ui, signal);
  }
  if (!prepared.preflight.ready)
    throw new ImportValidationError(
      `Unresolved references: ${prepared.preflight.missingReferences.map((m: any) => m.reference).join(", ")}`,
    );
  const files: string[] = prepared.preflight.files;
  if (
    !(await ui.confirm(
      `Загрузить в базу «${request.infobase}» из ${request.sourceDir}: ${files.length} файлов?\n${files.slice(0, 12).join("\n")}\nИзменения останутся в CONFIGSAVE до применения.`,
    ))
  )
    return;
  await check(ui, signal);
  return rpc.request(
    "objects/import",
    {
      infobase: request.infobase,
      sourceDir: request.sourceDir,
      plan: prepared.plan,
      confirm: true,
    },
    signal,
    600000,
  );
}
