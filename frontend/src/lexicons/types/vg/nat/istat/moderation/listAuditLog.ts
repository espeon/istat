import type {} from "@atcute/lexicons";
import * as v from "@atcute/lexicons/validations";
import type {} from "@atcute/lexicons/ambient";

const _auditLogEntrySchema = /*#__PURE__*/ v.object({
  $type: /*#__PURE__*/ v.optional(
    /*#__PURE__*/ v.literal(
      "vg.nat.istat.moderation.listAuditLog#auditLogEntry",
    ),
  ),
  /**
   * The moderation action performed
   */
  action: /*#__PURE__*/ v.literalEnum([
    "blacklist_cid",
    "delete_emoji",
    "delete_status",
    "remove_blacklist",
  ]),
  /**
   * When this action was performed
   */
  createdAt: /*#__PURE__*/ v.datetimeString(),
  /**
   * Unique identifier for this log entry
   */
  id: /*#__PURE__*/ v.integer(),
  /**
   * DID of the moderator who performed the action
   */
  moderatorDid: /*#__PURE__*/ v.didString(),
  /**
   * Handle of the moderator (for display)
   */
  moderatorHandle: /*#__PURE__*/ v.optional(/*#__PURE__*/ v.handleString()),
  /**
   * Reason for the action (for blacklists)
   */
  reason: /*#__PURE__*/ v.optional(
    /*#__PURE__*/ v.literalEnum([
      "copyright",
      "gore",
      "harassment",
      "nudity",
      "other",
      "spam",
    ]),
  ),
  /**
   * Additional details about the reason
   */
  reasonDetails: /*#__PURE__*/ v.optional(/*#__PURE__*/ v.string()),
  /**
   * Identifier of the targeted content (CID or AT-URI)
   */
  targetId: /*#__PURE__*/ v.string(),
  /**
   * Type of content targeted
   */
  targetType: /*#__PURE__*/ v.literalEnum([
    "avatar",
    "banner",
    "emoji",
    "emoji_blob",
    "status",
  ]),
});
const _mainSchema = /*#__PURE__*/ v.query(
  "vg.nat.istat.moderation.listAuditLog",
  {
    params: /*#__PURE__*/ v.object({
      /**
       * Pagination cursor
       */
      cursor: /*#__PURE__*/ v.optional(/*#__PURE__*/ v.string()),
      /**
       * Maximum number of entries to return
       * @minimum 1
       * @maximum 100
       * @default 50
       */
      limit: /*#__PURE__*/ v.optional(
        /*#__PURE__*/ v.constrain(/*#__PURE__*/ v.integer(), [
          /*#__PURE__*/ v.integerRange(1, 100),
        ]),
        50,
      ),
    }),
    output: {
      type: "lex",
      schema: /*#__PURE__*/ v.object({
        /**
         * Pagination cursor for next page
         */
        cursor: /*#__PURE__*/ v.optional(/*#__PURE__*/ v.string()),
        get entries() {
          return /*#__PURE__*/ v.array(auditLogEntrySchema);
        },
      }),
    },
  },
);

type auditLogEntry$schematype = typeof _auditLogEntrySchema;
type main$schematype = typeof _mainSchema;

export interface auditLogEntrySchema extends auditLogEntry$schematype {}
export interface mainSchema extends main$schematype {}

export const auditLogEntrySchema = _auditLogEntrySchema as auditLogEntrySchema;
export const mainSchema = _mainSchema as mainSchema;

export interface AuditLogEntry
  extends v.InferInput<typeof auditLogEntrySchema> {}

export interface $params extends v.InferInput<mainSchema["params"]> {}
export interface $output extends v.InferXRPCBodyInput<mainSchema["output"]> {}

declare module "@atcute/lexicons/ambient" {
  interface XRPCQueries {
    "vg.nat.istat.moderation.listAuditLog": mainSchema;
  }
}
