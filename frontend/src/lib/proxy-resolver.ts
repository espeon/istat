import type {
  IdentityResolver,
  ResolvedIdentity,
  ResolveIdentityOptions,
} from "@atcute/oauth-browser-client";
import type { ActorIdentifier } from "@atcute/lexicons";

/**
 * Wraps an existing identity resolver and rewrites PDS endpoints to point to our OAuth proxy
 */
export class ProxyIdentityResolver implements IdentityResolver {
  constructor(
    private upstream: IdentityResolver,
    private proxyUrl: string,
  ) {}

  async resolve(
    actor: ActorIdentifier,
    options?: ResolveIdentityOptions,
  ): Promise<ResolvedIdentity> {
    // Use the upstream resolver to get the actual identity
    const identity = await this.upstream.resolve(actor, options);

    // Rewrite the PDS endpoint to point to our proxy
    console.log(
      "Rewriting PDS endpoint from",
      identity.pds,
      "to",
      this.proxyUrl,
    );

    return {
      ...identity,
      pds: this.proxyUrl,
    };
  }
}
