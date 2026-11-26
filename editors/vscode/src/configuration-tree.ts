import * as fs from "node:fs/promises";
import * as path from "node:path";
import { DOMParser, Element } from "@xmldom/xmldom";

export interface Node {
  id: string;
  label: string;
  kind: string;
  type: "object" | "group" | "file" | "property";
  path?: string;
  fullName?: string;
  synonym?: string;
  children?: Node[];
  parent?: Node;
}
export interface Source {
  key: string;
  directory: string;
  remote: boolean;
  children(node: Node | undefined, signal: AbortSignal): Promise<Node[]>;
  read(file: string, signal: AbortSignal): Promise<Buffer | undefined>;
}
export interface Rpc {
  request(
    method: string,
    params?: object,
    signal?: AbortSignal,
    timeout?: number,
  ): Promise<any>;
}

// Loading complete source snapshots can take minutes; cancellation remains immediate.
export const SOURCE_OPERATION_TIMEOUT = 10 * 60 * 1000;

const plurals: Record<string, string> = {
  Configuration: "",
  Subsystem: "Subsystems",
  CommonModule: "CommonModules",
  CommonAttribute: "CommonAttributes",
  SessionParameter: "SessionParameters",
  Role: "Roles",
  CommonTemplate: "CommonTemplates",
  FilterCriterion: "FilterCriteria",
  ExchangePlan: "ExchangePlans",
  XDTOPackage: "XDTOPackages",
  WebService: "WebServices",
  HTTPService: "HTTPServices",
  WSReference: "WSReferences",
  EventSubscription: "EventSubscriptions",
  ScheduledJob: "ScheduledJobs",
  CommonForm: "CommonForms",
  CommonCommand: "CommonCommands",
  CommandGroup: "CommandGroups",
  CommonPicture: "CommonPictures",
  StyleItem: "StyleItems",
  Style: "Styles",
  Language: "Languages",
  DefinedType: "DefinedTypes",
  Constant: "Constants",
  Catalog: "Catalogs",
  Document: "Documents",
  DocumentNumerator: "DocumentNumerators",
  Sequence: "Sequences",
  DocumentJournal: "DocumentJournals",
  Enum: "Enums",
  Report: "Reports",
  DataProcessor: "DataProcessors",
  ChartOfCharacteristicTypes: "ChartsOfCharacteristicTypes",
  ChartOfAccounts: "ChartsOfAccounts",
  ChartOfCalculationTypes: "ChartsOfCalculationTypes",
  InformationRegister: "InformationRegisters",
  AccumulationRegister: "AccumulationRegisters",
  AccountingRegister: "AccountingRegisters",
  CalculationRegister: "CalculationRegisters",
  BusinessProcess: "BusinessProcesses",
  Task: "Tasks",
  ExternalDataSource: "ExternalDataSources",
  Form: "Forms",
  Template: "Templates",
  Command: "Commands",
  Recalculation: "Recalculations",
  Table: "Tables",
  Cube: "Cubes",
  IntegrationService: "IntegrationServices",
  WebSocketClient: "WebSocketClients",
  Bot: "Bots",
  SettingsStorage: "SettingsStorages",
  FunctionalOption: "FunctionalOptions",
  FunctionalOptionsParameter: "FunctionalOptionsParameters",
  Interface: "Interfaces",
  PaletteColor: "PaletteColors",
};
export const labels: Record<string, string> = {
  Configuration: "Конфигурация",
  Common: "Общие",
  Subsystem: "Подсистемы",
  CommonModule: "Общие модули",
  CommonAttribute: "Общие реквизиты",
  SessionParameter: "Параметры сеанса",
  Role: "Роли",
  CommonTemplate: "Общие макеты",
  FilterCriterion: "Критерии отбора",
  ExchangePlan: "Планы обмена",
  XDTOPackage: "Пакеты XDTO",
  WebService: "Веб-сервисы",
  HTTPService: "HTTP-сервисы",
  WSReference: "Ссылки WS",
  EventSubscription: "Подписки на события",
  ScheduledJob: "Регламентные задания",
  CommonForm: "Общие формы",
  CommonCommand: "Общие команды",
  CommandGroup: "Группы команд",
  CommonPicture: "Общие картинки",
  StyleItem: "Элементы стиля",
  Style: "Стили",
  Language: "Языки",
  DefinedType: "Определяемые типы",
  Constant: "Константы",
  Catalog: "Справочники",
  Document: "Документы",
  DocumentNumerator: "Нумераторы документов",
  Sequence: "Последовательности",
  DocumentJournal: "Журналы документов",
  Enum: "Перечисления",
  Report: "Отчёты",
  DataProcessor: "Обработки",
  ChartOfCharacteristicTypes: "Планы видов характеристик",
  ChartOfAccounts: "Планы счетов",
  ChartOfCalculationTypes: "Планы видов расчёта",
  InformationRegister: "Регистры сведений",
  AccumulationRegister: "Регистры накопления",
  AccountingRegister: "Регистры бухгалтерии",
  CalculationRegister: "Регистры расчёта",
  BusinessProcess: "Бизнес-процессы",
  Task: "Задачи",
  ExternalDataSource: "Внешние источники данных",
  Form: "Формы",
  Template: "Макеты",
  Command: "Команды",
  Recalculation: "Перерасчёты",
  Attribute: "Реквизиты",
  TabularSection: "Табличные части",
  Dimension: "Измерения",
  Resource: "Ресурсы",
  EnumValue: "Значения",
  AccountingFlag: "Признаки учёта",
  ExtDimensionAccountingFlag: "Признаки учёта субконто",
  Table: "Таблицы",
  Field: "Поля",
  Cube: "Кубы",
  IntegrationService: "Сервисы интеграции",
  WebSocketClient: "Клиенты WebSocket",
  Bot: "Боты",
  URLTemplate: "Шаблоны URL",
  Method: "Методы",
  Operation: "Операции",
  Parameter: "Параметры",
  AddressingAttribute: "Реквизиты адресации",
  SettingsStorage: "Хранилища настроек",
  FunctionalOption: "Функциональные опции",
  FunctionalOptionsParameter: "Параметры функциональных опций",
  Interface: "Интерфейсы",
  PaletteColor: "Цвета палитры",
};
const common = new Set([
  "Subsystem",
  "CommonModule",
  "CommonAttribute",
  "SessionParameter",
  "Role",
  "CommonTemplate",
  "FilterCriterion",
  "ExchangePlan",
  "XDTOPackage",
  "WebService",
  "HTTPService",
  "WSReference",
  "WebSocketClient",
  "EventSubscription",
  "ScheduledJob",
  "CommonForm",
  "CommonCommand",
  "CommandGroup",
  "CommonPicture",
  "StyleItem",
  "Style",
  "Language",
  "DefinedType",
  "SettingsStorage",
  "FunctionalOption",
  "FunctionalOptionsParameter",
  "IntegrationService",
  "Bot",
  "Interface",
  "PaletteColor",
]);
export function relativePath(file: string): string {
  if (
    !file ||
    file.includes("\\") ||
    /[:\x00-\x1f]/.test(file) ||
    file.split("/").some((p) => !p || p === "." || p === "..")
  )
    throw new Error("Недопустимый путь исходного файла");
  return file;
}
export function localPath(root: string, file: string): string {
  return path.join(root, ...relativePath(file).split("/"));
}
function name(value: string): string {
  if (
    !value ||
    /[\\/:\x00-\x1f]/.test(value) ||
    value === "." ||
    value === ".."
  )
    throw new Error("Недопустимое имя объекта метаданных");
  return value;
}
function elements(e: Element): Element[] {
  return Array.from(e.childNodes).filter((n) => n.nodeType === 1) as Element[];
}
function child(e: Element, tag: string): Element | undefined {
  return elements(e).find(
    (n) => n.namespaceURI === e.namespaceURI && n.localName === tag,
  );
}
function text(e: Element | undefined): string {
  return e?.textContent ?? "";
}
function property(e: Element, tag: string): string {
  const p = child(e, "Properties");
  return p ? text(child(p, tag)) : "";
}
function synonym(e: Element): string {
  const p = child(e, "Properties");
  const s = p && child(p, "Synonym");
  return s
    ? Array.from(s.getElementsByTagNameNS("*", "content")).map(text).join(" · ")
    : "";
}
function group(owner: Node, kind: string, children: Node[]): Node {
  return {
    id: `${owner.id}/${plurals[kind] ?? kind}`,
    label: labels[kind] ?? kind,
    type: "group",
    kind,
    children,
  };
}
export function arrange(owner: Node, nodes: Node[]): Node[] {
  if (owner.kind !== "Configuration") return nodes;
  const general = nodes.filter((n) => n.type === "group" && common.has(n.kind));
  return general.length
    ? [
        group(owner, "Common", general),
        ...nodes.filter((n) => !general.includes(n)),
      ]
    : nodes;
}
export function groupContains(group: Node, fullName: string): boolean {
  if (group.kind === "Common") return common.has(fullName.split(".")[0]);

  let owner = group.parent;
  while (owner?.type === "group") owner = owner.parent;
  const prefix =
    owner?.fullName && owner.kind !== "Configuration"
      ? `${owner.fullName}.`
      : "";
  return fullName.startsWith(`${prefix}${group.kind}.`);
}

export function descriptor(
  bytes: Buffer,
  owner: Node,
  files: string[],
): Node[] {
  const xml = bytes.toString("utf8").replace(/^\uFEFF/, "");
  if (/<!DOCTYPE/i.test(xml))
    throw new Error("Объявление DTD в метаданных не поддерживается");
  const doc = new DOMParser({
    onError: () => {
      throw new Error("Некорректный XML метаданных");
    },
  }).parseFromString(xml, "application/xml");
  const root = doc.documentElement;
  if (!root || root.localName !== "MetaDataObject")
    throw new Error("Ожидалось описание объекта метаданных");
  const object = elements(root).find(
    (n) => n.namespaceURI === root.namespaceURI,
  );
  if (!object || object.localName !== owner.kind)
    throw new Error("Вид объекта не соответствует описанию");
  const metadataNamespace = object.namespaceURI;
  owner.synonym = synonym(object);
  const folder =
    owner.kind === "Configuration" ? "" : owner.path!.replace(/\.xml$/, "");
  function contained(e: Element, parent: Node, directory: string): Node[] {
    const groups = new Map<string, Node[]>();
    const container = child(e, "ChildObjects");
    for (const item of container ? elements(container) : []) {
      if (item.namespaceURI !== metadataNamespace) continue;

      const kind = item.localName!;
      const embedded = !!child(item, "Properties");
      const label = name(embedded ? property(item, "Name") : text(item).trim());
      const fullName =
        parent.kind === "Configuration"
          ? `${kind}.${label}`
          : `${parent.fullName}.${kind}.${label}`;
      let node: Node;
      if (embedded) {
        node = {
          id: fullName,
          label,
          kind,
          type: "property",
          path: owner.path,
          synonym: synonym(item),
          fullName,
        };
        node.children = contained(item, node, directory);
        if (kind === "Command") {
          const prefix =
            [directory, "Commands", label, "Ext"].filter(Boolean).join("/") +
            "/";
          for (const file of files
            .filter((file) => file.startsWith(prefix))
            .sort()) {
            node.children.push({
              id: `${fullName}:file:${file}`,
              label: file.slice(prefix.length),
              kind: "file",
              type: "file",
              path: relativePath(file),
            });
          }
        }
      } else {
        const collection = plurals[kind];
        if (!collection)
          throw new Error(`Неизвестная коллекция метаданных: ${kind}`);
        const file = [directory, collection, label + ".xml"]
          .filter(Boolean)
          .join("/");
        node = {
          id: fullName,
          fullName,
          label,
          kind,
          type: "object",
          path: relativePath(file),
        };
      }
      const list = groups.get(kind) ?? [];
      list.push(node);
      groups.set(kind, list);
    }
    return [...groups].map(([kind, list]) => group(parent, kind, list));
  }
  const result = contained(object, owner, folder);
  const prefix = folder ? folder + "/Ext/" : "Ext/";
  for (const file of files.filter((f) => f.startsWith(prefix)).sort()) {
    relativePath(file);
    result.push({
      id: `${owner.id}:file:${file}`,
      label: file.slice(prefix.length),
      kind: "file",
      type: "file",
      path: file,
    });
  }
  return arrange(owner, result);
}
const rootNode = (): Node => ({
  id: "Configuration",
  label: "Конфигурация",
  kind: "Configuration",
  type: "object",
  path: "Configuration.xml",
  fullName: "Configuration",
});

export class XmlSource implements Source {
  remote = false;
  key: string;
  constructor(public directory: string) {
    this.directory = path.resolve(directory);
    this.key = this.directory;
  }
  async read(file: string, signal: AbortSignal): Promise<Buffer | undefined> {
    const target = localPath(this.directory, file);
    let realRoot: string;
    try {
      realRoot = await fs.realpath(this.directory);
    } catch (e: any) {
      if (e.code === "ENOENT") return;
      throw e;
    }
    let real: string;
    try {
      real = await fs.realpath(target);
    } catch (e: any) {
      if (e.code === "ENOENT") return;
      throw e;
    }
    const relative = path.relative(realRoot, real);
    if (
      relative === ".." ||
      relative.startsWith(".." + path.sep) ||
      path.isAbsolute(relative)
    )
      throw new Error("Файл находится вне каталога исходников");
    return fs.readFile(real, { signal });
  }
  async children(node: Node | undefined, signal: AbortSignal): Promise<Node[]> {
    if (!node) {
      if (!(await this.read("Configuration.xml", signal)))
        throw new Error("Configuration.xml не найден");
      return [rootNode()];
    }
    if (node.children) return node.children;
    if (node.type !== "object") return [];
    const bytes = await this.read(node.path!, signal);
    if (!bytes) throw new Error(`Описание ${node.path} не найдено`);
    const prefix =
      node.kind === "Configuration"
        ? "Ext"
        : node.path!.replace(/\.xml$/, "") + "/Ext";
    const files: string[] = [];
    const walk = async (dir: string) => {
      if (signal.aborted) throw new Error("Чтение отменено");
      let entries;
      try {
        const resolved = await fs.realpath(localPath(this.directory, dir));
        const relative = path.relative(
          await fs.realpath(this.directory),
          resolved,
        );
        if (
          relative === ".." ||
          relative.startsWith(".." + path.sep) ||
          path.isAbsolute(relative)
        )
          throw new Error("Каталог находится вне дерева исходников");
        entries = await fs.readdir(localPath(this.directory, dir), {
          withFileTypes: true,
        });
      } catch (e: any) {
        if (e.code === "ENOENT") return;
        throw e;
      }
      for (const entry of entries) {
        const file = dir + "/" + entry.name;
        if (entry.isSymbolicLink())
          throw new Error("Ссылки в дереве исходников не поддерживаются");
        if (entry.isDirectory()) await walk(file);
        else if (entry.isFile()) files.push(file);
      }
    };
    await walk(prefix);
    if (node.kind !== "Configuration")
      await walk(node.path!.replace(/\.xml$/, "") + "/Commands");
    return descriptor(bytes, node, files);
  }
}
export function readResult(value: any, file: string): Buffer | undefined {
  if (value?.path !== file || typeof value.exists !== "boolean")
    throw new Error("Неверный ответ чтения файла");
  if (!value.exists) return;
  if (typeof value.content !== "string")
    throw new Error("Нет содержимого файла");
  if (value.encoding === "utf-8") return Buffer.from(value.content, "utf8");
  if (
    value.encoding === "base64" &&
    /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(
      value.content,
    )
  )
    return Buffer.from(value.content, "base64");
  throw new Error("Неизвестная кодировка файла");
}
export class DatabaseSource implements Source {
  remote = true;
  key: string;
  constructor(
    public rpc: Rpc,
    public directory: string,
    public infobase: string,
  ) {
    this.key = JSON.stringify([directory, infobase]);
  }
  async read(file: string, signal: AbortSignal): Promise<Buffer | undefined> {
    relativePath(file);
    return readResult(
      await this.rpc.request(
        "source/read",
        { infobase: this.infobase, path: file },
        signal,
        SOURCE_OPERATION_TIMEOUT,
      ),
      file,
    );
  }
  async children(node: Node | undefined, signal: AbortSignal): Promise<Node[]> {
    if (node?.children) return node.children;
    if (node?.kind === "ConfigurationFiles") {
      const result = await this.rpc.request(
        "source/read",
        { infobase: this.infobase, path: "Configuration.xml" },
        signal,
        SOURCE_OPERATION_TIMEOUT,
      );
      const bytes = readResult(result, "Configuration.xml");
      if (
        !bytes ||
        !Array.isArray(result.files) ||
        !result.files.every((f: unknown) => typeof f === "string")
      )
        throw new Error("Сервер не вернул состав файлов конфигурации");
      return descriptor(bytes, rootNode(), result.files).filter(
        (n) => n.type === "file",
      );
    }
    if (node && node.type !== "group" && node.kind !== "Configuration") {
      if (node.type !== "object") return [];
      const value = await this.rpc.request(
        "source/read",
        { infobase: this.infobase, path: node.path },
        signal,
        SOURCE_OPERATION_TIMEOUT,
      );
      const bytes = readResult(value, node.path!);
      if (
        !bytes ||
        !Array.isArray(value.files) ||
        !value.files.every((f: unknown) => typeof f === "string")
      )
        throw new Error(
          "Сервер не вернул состав файлов объекта; обновите ibcmd-rs",
        );
      return descriptor(bytes, node, value.files);
    }
    const response = await this.rpc.request(
      "tree/children",
      { infobase: this.infobase, ...(node ? { node: node.id } : {}) },
      signal,
    );
    if (!Array.isArray(response?.nodes))
      throw new Error("Неверный ответ дерева");
    const nodes = response.nodes.map((n: any): Node => {
      if (
        typeof n.id !== "string" ||
        typeof n.label !== "string" ||
        typeof n.kind !== "string" ||
        !["group", "configuration", "object"].includes(n.nodeType)
      )
        throw new Error("Неверный узел дерева");
      return {
        id: n.id,
        label: n.nodeType === "group" ? (labels[n.kind] ?? n.label) : n.label,
        kind: n.kind,
        type: n.nodeType === "group" ? "group" : "object",
        fullName: n.fullName,
        path: typeof n.path === "string" ? relativePath(n.path) : undefined,
      };
    });
    if (node?.kind === "Configuration")
      nodes.push({
        id: "Configuration:files",
        label: "Модули и настройки",
        kind: "ConfigurationFiles",
        type: "group",
      });
    return node ? arrange(node, nodes) : nodes;
  }
}

export class TreeModel {
  private cache = new Map<
    string,
    {
      value: Promise<Node[]>;
      abort: AbortController;
      readers: number;
      complete: boolean;
    }
  >();
  private abort = new AbortController();
  constructor(public source: Source) {}
  dispose() {
    this.abort.abort();
    for (const entry of this.cache.values()) entry.abort.abort();
    this.cache.clear();
  }
  async children(node?: Node, signal?: AbortSignal): Promise<Node[]> {
    if (signal?.aborted) throw new Error("Поиск отменён");
    if (this.abort.signal.aborted) return [];

    const key = node?.id ?? "";
    let entry = this.cache.get(key);
    if (!entry) {
      const pending = {
        value: Promise.resolve<Node[]>([]),
        abort: new AbortController(),
        readers: 0,
        complete: false,
      };
      pending.value = this.source
        .children(node, pending.abort.signal)
        .then((nodes) => {
          if (this.abort.signal.aborted) return [];
          if (pending.abort.signal.aborted) throw new Error("Чтение отменено");

          const ids = new Set<string>();
          for (const child of nodes) {
            if (ids.has(child.id)) throw new Error("Повторяющийся узел дерева");
            for (let ancestor = node; ancestor; ancestor = ancestor.parent)
              if (ancestor.id === child.id)
                throw new Error("Циклическое дерево");
            ids.add(child.id);
            child.parent = node;
          }
          return nodes;
        })
        .catch((error) => {
          if (this.cache.get(key) === pending) this.cache.delete(key);
          throw error;
        })
        .finally(() => {
          pending.complete = true;
        });
      entry = pending;
      this.cache.set(key, entry);
    }
    const read = entry;
    read.readers++;

    return new Promise<Node[]>((resolve, reject) => {
      let finished = false;
      const release = () => {
        if (finished) return false;
        finished = true;
        signal?.removeEventListener("abort", cancel);
        read.readers--;
        if (!read.readers && !read.complete) {
          if (this.cache.get(key) === read) this.cache.delete(key);
          read.abort.abort();
        }
        return true;
      };
      const cancel = () => {
        if (release()) reject(new Error("Поиск отменён"));
      };
      signal?.addEventListener("abort", cancel, { once: true });
      read.value.then(
        (nodes) => {
          if (release()) resolve(nodes);
        },
        (error) => {
          if (release()) reject(error);
        },
      );
      if (signal?.aborted) cancel();
    });
  }
  async search(query: string, signal: AbortSignal): Promise<Node[]> {
    const found: Node[] = [];
    const q = query.toLocaleLowerCase("ru");
    const visit = async (node?: Node): Promise<void> => {
      if (signal.aborted || this.abort.signal.aborted)
        throw new Error("Поиск отменён");
      const children = await this.children(node, signal);
      if (signal.aborted || this.abort.signal.aborted)
        throw new Error("Поиск отменён");

      if (
        node &&
        node.type !== "group" &&
        [node.label, node.synonym ?? ""].some((s) =>
          s.toLocaleLowerCase("ru").includes(q),
        )
      )
        found.push(node);
      for (const child of children)
        if (child.type !== "file") await visit(child);
    };
    await visit();
    return found;
  }
}
