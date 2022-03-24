/** Shared input/result type helpers for generated delegates (use-site reference paths). */

type EntityByName = {
{{ENTITY_MAP}}
};

type EntityName = {{ENTITY_UNION}};

/** Nested select shape for a reference target (use-site dereference projection). */
export type SelectPathFor<E extends EntityName> = {
    [K in keyof EntityByName[E]]?:
        | boolean
        | (EntityByName[E][K] extends { readonly __irisRef: infer T extends EntityName }
              ? SelectPathFor<T>
              : never);
};

/** Nested where path for a reference target (use-site filter navigation). */
export type WherePathFor<E extends EntityName> = {
    [K in keyof EntityByName[E]]?:
        | WhereValueForField<EntityByName[E][K]>
        | (EntityByName[E][K] extends { readonly __irisRef: infer T extends EntityName }
              ? WherePathFor<T>
              : never);
};

type WhereValueForField<V> = V extends { readonly __irisRef: EntityName }
    ? never
    : V | FilterOps<V extends null | undefined ? NonNullable<V> : V>;

type FilterOps<T> = {
    eq?: T;
    not?: T;
    gt?: T;
    gte?: T;
    lt?: T;
    lte?: T;
    contains?: string;
    startsWith?: string;
    endsWith?: string;
};

/**
 * Resolve a select shape against an entity — nested object on &T becomes
 * SelectResultFor of the target; `true` on &T means full target entity.
 */
export type SelectResultFor<E extends EntityName, S> = [S] extends [undefined]
    ? EntityByName[E]
    : {
          [K in keyof S & keyof EntityByName[E] as S[K] extends true | object ? K : never]: S[K] extends true
              ? EntityByName[E][K] extends { readonly __irisRef: infer T extends EntityName }
                  ? EntityByName[T]
                  : EntityByName[E][K]
              : S[K] extends object
                ? EntityByName[E][K] extends { readonly __irisRef: infer T extends EntityName }
                    ? SelectResultFor<T, S[K]>
                    : never
                : never;
      };
