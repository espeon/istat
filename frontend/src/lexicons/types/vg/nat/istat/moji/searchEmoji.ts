import type {} from "@atcute/lexicons";
import * as v from "@atcute/lexicons/validations";
import type {} from "@atcute/lexicons/ambient";

const _emojiViewSchema = /*#__PURE__*/ v.object({
  $type: /*#__PURE__*/ v.optional(
    /*#__PURE__*/ v.literal("vg.nat.istat.moji.searchEmoji#emojiView"),
  ),
  /**
   * Alt text description
   */
  altText: /*#__PURE__*/ v.optional(/*#__PURE__*/ v.string()),
  /**
   * DID of the user who created this emoji
   */
  createdBy: /*#__PURE__*/ v.didString(),
  /**
   * Handle of the user who created this emoji
   */
  createdByHandle: /*#__PURE__*/ v.optional(/*#__PURE__*/ v.handleString()),
  /**
   * Canonical name of the emoji
   */
  name: /*#__PURE__*/ v.string(),
  /**
   * AT-URI of the emoji record
   */
  uri: /*#__PURE__*/ v.resourceUriString(),
  /**
   * URL to the emoji image
   */
  url: /*#__PURE__*/ v.string(),
});
const _mainSchema = /*#__PURE__*/ v.query("vg.nat.istat.moji.searchEmoji", {
  params: /*#__PURE__*/ v.object({
    /**
     * Maximum number of emojis to return
     * @minimum 1
     * @maximum 100
     * @default 20
     */
    limit: /*#__PURE__*/ v.optional(
      /*#__PURE__*/ v.constrain(/*#__PURE__*/ v.integer(), [
        /*#__PURE__*/ v.integerRange(1, 100),
      ]),
      20,
    ),
    /**
     * Search query to match against emoji name and alt text
     */
    query: /*#__PURE__*/ v.string(),
  }),
  output: {
    type: "lex",
    schema: /*#__PURE__*/ v.object({
      get emojis() {
        return /*#__PURE__*/ v.array(emojiViewSchema);
      },
    }),
  },
});

type emojiView$schematype = typeof _emojiViewSchema;
type main$schematype = typeof _mainSchema;

export interface emojiViewSchema extends emojiView$schematype {}
export interface mainSchema extends main$schematype {}

export const emojiViewSchema = _emojiViewSchema as emojiViewSchema;
export const mainSchema = _mainSchema as mainSchema;

export interface EmojiView extends v.InferInput<typeof emojiViewSchema> {}

export interface $params extends v.InferInput<mainSchema["params"]> {}
export interface $output extends v.InferXRPCBodyInput<mainSchema["output"]> {}

declare module "@atcute/lexicons/ambient" {
  interface XRPCQueries {
    "vg.nat.istat.moji.searchEmoji": mainSchema;
  }
}
