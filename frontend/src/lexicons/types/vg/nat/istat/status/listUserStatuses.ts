import type {} from "@atcute/lexicons";
import * as v from "@atcute/lexicons/validations";
import type {} from "@atcute/lexicons/ambient";

const _mainSchema = /*#__PURE__*/ v.query(
  "vg.nat.istat.status.listUserStatuses",
  {
    params: /*#__PURE__*/ v.object({
      /**
       * Pagination cursor
       */
      cursor: /*#__PURE__*/ v.optional(/*#__PURE__*/ v.string()),
      /**
       * The handle of the user whose status history to retrieve
       */
      handle: /*#__PURE__*/ v.handleString(),
      /**
       * Maximum number of statuses to return
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
        get statuses() {
          return /*#__PURE__*/ v.array(userStatusViewSchema);
        },
      }),
    },
  },
);
const _userStatusViewSchema = /*#__PURE__*/ v.object({
  $type: /*#__PURE__*/ v.optional(
    /*#__PURE__*/ v.literal(
      "vg.nat.istat.status.listUserStatuses#userStatusView",
    ),
  ),
  /**
   * URL to the user's avatar image
   */
  avatarUrl: /*#__PURE__*/ v.optional(/*#__PURE__*/ v.string()),
  /**
   * When this status was created
   */
  createdAt: /*#__PURE__*/ v.datetimeString(),
  /**
   * Optional status text description
   * @maxLength 20480
   * @maxGraphemes 2048
   */
  description: /*#__PURE__*/ v.optional(
    /*#__PURE__*/ v.constrain(/*#__PURE__*/ v.string(), [
      /*#__PURE__*/ v.stringLength(0, 20480),
      /*#__PURE__*/ v.stringGraphemes(0, 2048),
    ]),
  ),
  /**
   * The user's display name from their profile
   * @maxLength 640
   * @maxGraphemes 64
   */
  displayName: /*#__PURE__*/ v.optional(
    /*#__PURE__*/ v.constrain(/*#__PURE__*/ v.string(), [
      /*#__PURE__*/ v.stringLength(0, 640),
      /*#__PURE__*/ v.stringGraphemes(0, 64),
    ]),
  ),
  /**
   * Alt text for the emoji
   */
  emojiAlt: /*#__PURE__*/ v.optional(/*#__PURE__*/ v.string()),
  /**
   * Canonical name/identifier for the emoji (no spaces, e.g. 'POGGERS', 'Cinema')
   */
  emojiName: /*#__PURE__*/ v.optional(/*#__PURE__*/ v.string()),
  /**
   * URL to the emoji
   */
  emojiUrl: /*#__PURE__*/ v.string(),
  /**
   * Optional expiration timestamp
   */
  expires: /*#__PURE__*/ v.optional(/*#__PURE__*/ v.datetimeString()),
  /**
   * The user's handle
   */
  handle: /*#__PURE__*/ v.optional(/*#__PURE__*/ v.handleString()),
  /**
   * The record key
   */
  rkey: /*#__PURE__*/ v.string(),
  /**
   * Optional status text title
   * @maxLength 2560
   * @maxGraphemes 256
   */
  title: /*#__PURE__*/ v.optional(
    /*#__PURE__*/ v.constrain(/*#__PURE__*/ v.string(), [
      /*#__PURE__*/ v.stringLength(0, 2560),
      /*#__PURE__*/ v.stringGraphemes(0, 256),
    ]),
  ),
});

type main$schematype = typeof _mainSchema;
type userStatusView$schematype = typeof _userStatusViewSchema;

export interface mainSchema extends main$schematype {}
export interface userStatusViewSchema extends userStatusView$schematype {}

export const mainSchema = _mainSchema as mainSchema;
export const userStatusViewSchema =
  _userStatusViewSchema as userStatusViewSchema;

export interface UserStatusView
  extends v.InferInput<typeof userStatusViewSchema> {}

export interface $params extends v.InferInput<mainSchema["params"]> {}
export interface $output extends v.InferXRPCBodyInput<mainSchema["output"]> {}

declare module "@atcute/lexicons/ambient" {
  interface XRPCQueries {
    "vg.nat.istat.status.listUserStatuses": mainSchema;
  }
}
