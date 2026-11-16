// What the two names of a referenced object share (`ObjectName`, read from its record;
// `SummaryName`, from the summary the server gave, task 021-11p10-d1): its collection, by
// its kind or by the one its URL names.
import { collectionById, collectionOfKind, type Collection } from "../collections.ts";

/** The collection of an object named by `kind`, else by its URL `href`. */
export function collectionOfRef(
  kind: string | null | undefined,
  href: string | null | undefined,
): Collection | undefined {
  return collectionOfKind(kind) ?? collectionById(/\/v1\/([a-z-]+)\//.exec(href ?? "")?.[1]);
}
