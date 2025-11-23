import type {} from "@atcute/lexicons";
import * as v from "@atcute/lexicons/validations";
import type {} from "@atcute/lexicons/ambient";

const _blacklistedCidViewSchema = /*#__PURE__*/ v.object({
  $type: /*#__PURE__*/ v.optional(
    /*#__PURE__*/ v.literal(
      "vg.nat.istat.moderation.listBlacklisted#blacklistedCidView",
    ),
  ),
  /**
   * When this was blacklisted
   */
  blacklistedAt: /*#__PURE__*/ v.datetimeString(),
  /**
   * The blacklisted CID
   */
  cid: /*#__PURE__*/ v.string(),
  /**
   * Type of content
   */
  contentType: /*#__PURE__*/ v.string(),
  /**
   * DID of the moderator who blacklisted this
   */
  moderatorDid: /*#__PURE__*/ v.didString(),
  /**
   * Reason for blacklisting
   */
  reason: /*#__PURE__*/ v.string(),
  /**
   * Additional details about the blacklist
   */
  reasonDetails: /*#__PURE__*/ v.optional(/*#__PURE__*/ v.string()),
});
const _mainSchema = /*#__PURE__*/ v.query(
  "vg.nat.istat.moderation.listBlacklisted",
  {
    params: /*#__PURE__*/ v.object({
      /**
       * Filter by content type
       */
      contentType: /*#__PURE__*/ v.optional(
        /*#__PURE__*/ v.literalEnum(["avatar", "banner", "emoji_blob"]),
      ),
      /**
       * Maximum number of blacklisted CIDs to return
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
        get blacklisted() {
          return /*#__PURE__*/ v.array(blacklistedCidViewSchema);
        },
      }),
    },
  },
);

type blacklistedCidView$schematype = typeof _blacklistedCidViewSchema;
type main$schematype = typeof _mainSchema;

export interface blacklistedCidViewSchema
  extends blacklistedCidView$schematype {}
export interface mainSchema extends main$schematype {}

export const blacklistedCidViewSchema =
  _blacklistedCidViewSchema as blacklistedCidViewSchema;
export const mainSchema = _mainSchema as mainSchema;

export interface BlacklistedCidView
  extends v.InferInput<typeof blacklistedCidViewSchema> {}

export interface $params extends v.InferInput<mainSchema["params"]> {}
export interface $output extends v.InferXRPCBodyInput<mainSchema["output"]> {}

declare module "@atcute/lexicons/ambient" {
  interface XRPCQueries {
    "vg.nat.istat.moderation.listBlacklisted": mainSchema;
  }
}
