import { ContextActions } from "../../bindings/ContextActions";
import { SearchResult } from "../../bindings/SearchResult";
import { SearchResultTemplate } from "../../bindings/SearchResultTemplate";

export function includeAction(
  contextAction: ContextActions,
  selectedRow: SearchResult,
): boolean {
  const context = contextAction.context;
  if (
    (context.exclude_tag && containsTag(selectedRow, context.exclude_tag)) ||
    (context.exclude_tag_type &&
      containsTagType(selectedRow, context.exclude_tag_type))
  ) {
    return false;
  } else if (
    (context.has_tag && containsTag(selectedRow, context.has_tag)) ||
    (context.has_tag_type &&
      containsTagType(selectedRow, context.has_tag_type)) ||
    (context.url_like && containsUrl(selectedRow, context.url_like))
  ) {
    return true;
  }
  return false;
}

export function containsUrl(row: SearchResult, url_regex: string[]): boolean {
  return url_regex.some((regexStr) => new RegExp(regexStr).test(row.url));
}

export function containsTag(
  row: SearchResult,
  tags: [string, string][],
): boolean {
  const rowTagSet = new Set(row.tags.map((pair) => pair.join(",")));
  return tags.some((pair) => rowTagSet.has(pair.join(",")));
}

export function containsTagType(row: SearchResult, types: string[]): boolean {
  const rowTagTypes = new Set(row.tags.map(([tagType]) => tagType));
  return types.some((tagType) => rowTagTypes.has(tagType));
}

export function resultToTemplate(result: SearchResult) {
  let open_url = result.url;
  if (result.url.startsWith("file:")) {
    open_url = url_to_file_path(open_url);
  }

  let url_parent = "";
  const index = result.url.lastIndexOf("/");
  if (index >= 0) {
    url_parent = result.url.substring(0, index);
  }

  let url_schema = "";
  let url_userinfo = "";
  let url_port = 0;
  let url_path: string[] = [];
  let url_path_length = 0;
  let url_query = "";
  const parsed_url = URL.parse(result.url);
  if (parsed_url) {
    url_schema = parsed_url.protocol;
    url_userinfo = parsed_url.username;
    if (parsed_url.port !== "") {
      url_port = Number.parseInt(parsed_url.port);
    }
    url_path = parsed_url.pathname.split("/");
    url_path_length = url_path.length;
    url_query = parsed_url.search;
  }

  return {
    doc_id: result.doc_id,
    crawl_uri: result.crawl_uri,
    domain: result.domain,
    title: result.title,
    description: result.description,
    url: result.url,
    tags: result.tags,
    score: result.score,
    open_url: open_url,
    url_parent,
    url_schema,
    url_userinfo,
    url_port,
    url_path,
    url_path_length,
    url_query,
  } as SearchResultTemplate;
}

export function url_to_file_path(path: string) {
  let file_path = path.replace("%3A", ":").replace("%20", " ");

  if (path.startsWith("file:///")) {
    file_path = file_path.substring("file:///".length);
    // Convert path dividers into Windows specific ones.
    file_path = file_path.replace("/", "\\");
  }

  return file_path;
}
