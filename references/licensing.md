# Application licensing comparison

Checked 17 October 2026 against D03, D214 and D219–D221. D221 selects standard Elastic License 2.0 for Clacks’s own application code, without a free/community-hosting exception. The comparison explains that selection; compatibility assessments are not legal opinions on particular deployments. Third-party reference material keeps its existing licenses.

## Requirements and interpretation

Clacks Log must remain self-hostable, with an optional first-party hosted subscription. D219/D220 reserve hosting for others whether paid or free, while allowing individual personal self-hosting. D214 rejects the GNU General Public License (GPL) distribution route; D221 selects Elastic License 2.0. The requirements do not establish a blanket prohibition on business use, paid support, modifications, redistribution. D220 clarifies that free shared/community hosting is competing. Other permissions must not be silently treated as accepted decisions.

The recommendation favors preserving ordinary use, modification and redistribution while restricting hosted competition. That preference is a recommendation, not additional accepted scope. There is no accepted expiration date for the hosting reservation, so licenses that automatically lift it are a weaker fit.

A restriction on commercial hosting makes this source-available rather than open source under the [Open Source Initiative definition](https://opensource.org/osd), which disallows restrictions on fields of endeavor. Publicly readable source alone does not establish open-source licensing.

## Comparison

| Option | Self-hosting, changes and redistribution | Hosting reservation and tradeoff | Fit |
| --- | --- | --- | --- |
| [Elastic License 2.0](https://www.elastic.co/licensing/elastic-license/) | Allows use, modification and redistribution subject to its conditions, including ordinary internal business use. | Restricts third-party hosted/managed services exposing substantial software functionality. No automatic conversion to unrestricted terms. Also protects license-key mechanisms and notices. Its restriction is not limited to paid services, consistent with D220. | Closest existing standard license; selected under D221 with the clarified hosting boundary. |
| [PolyForm Perimeter 1.0.0](https://raw.githubusercontent.com/polyformproject/polyform-licenses/1.0.0/PolyForm-Perimeter-1.0.0.md) | Grants changes/distribution for permitted purposes. | Restricts providing competing products, including services and free products. Its competition test extends across deployment styles and interfaces. | Broader than reserving hosting: a distributed competing logger fork can also be affected. |
| [PolyForm Shield 1.0.0](https://raw.githubusercontent.com/polyformproject/polyform-licenses/1.0.0/PolyForm-Shield-1.0.0.md) | Grants changes/distribution for permitted purposes. | Covers competition with the software and other products the licensor/affiliates provide using it, including free offerings; includes rules for new and discontinued products. | Adds business-wide competition restrictions Clacks has not requested. |
| [Functional Source License 1.1](https://raw.githubusercontent.com/getsentry/fsl.software/main/FSL-1.1-ALv2.template.md) | Allows changes/distribution for permitted purposes, expressly including internal use and qualifying professional services. | Restricts competing commercial products/services, but each version becomes available under Apache License 2.0 after two years in this variant. An MIT-future-license variant also exists. | Good only if competition using older releases should eventually be allowed; that is not the current requirement. |
| [Business Source License 1.1](https://mariadb.com/bsl11/) | Base grant allows non-production use; an Additional Use Grant must permit normal production self-hosting. Changes/distribution are allowed subject to terms. | Can tailor production permissions, but each version changes to a qualifying open-source license by four years at the latest. The change license need not be GPL: a GPL-compatible alternative is possible. | More configuration and an eventual end to protection for each version. |
| [PolyForm Noncommercial 1.0.0](https://polyformproject.org/licenses/noncommercial/1.0.0) | Changes/distribution are available for permitted purposes, with specified personal and organizational permissions. | Restricts commercial purposes generally, rather than specifically hosted competition. | Too broad; D219 does not select a general noncommercial policy. |
| [Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0) | Broad rights to use, modify and redistribute, with notice and other conditions. | Does not reserve competing paid hosting. | Does not satisfy D219. |
| [Server Side Public License](https://www.mongodb.com/legal/licensing/server-side-public-license) | Allows use and modification under extensive source-sharing conditions. | Service provision triggers disclosure of the program and service infrastructure source; it does not categorically reserve hosting to the author. | Solves a different problem and imposes substantially broader sharing obligations. |
| Custom source-available license | Can express exactly the selected rights. | Can target hosting for others without an expiration, but requires drafting and maintaining the actual boundary. | Exact fit is possible, with greater legal and interpretive overhead than a standard text. |

The GNU Affero General Public License is also not a substitute for a hosting reservation: its network provision concerns source access for users, rather than exclusive commercial hosting. See the [GNU license overview](https://www.gnu.org/licenses/licenses.en.html). Do not reopen the rejected GPL direction by presenting source-sharing obligations as a commercial-hosting prohibition.

## Recommendation under the clarified hosting boundary

D221 selects **standard Elastic License 2.0**, rather than a general noncommercial/noncompete license, a license with automatic future conversion, or a bespoke full license. Its restriction targets the activity Clacks intends to reserve while its grants preserve useful source access and ordinary software freedoms. This is a fit judgment based on the cited text.

D220 resolves the previous free/shared-hosting caveat: hosting for others competes even without payment. The standard hosted-service restriction is therefore aligned with that policy; no supplementary community-hosting permission is recommended. Individual personal self-hosting remains the allowed case. This updates the earlier recommendation rather than treating a proposed free-hosting exception as accepted scope.

The [Elastic FAQ](https://www.elastic.co/licensing/elastic-license/faq) distinguishes paid installation help for internal use from providing the product itself as a managed service. Renting ordinary computing infrastructure is likewise a different activity from offering Clacks functionality; particular arrangements need to be judged on what is actually provided. The license-key clause does not require adding activation, paywalls or branding widgets to Clacks. Its notice obligations still apply, and modification/distribution does not mean removing all original notices.

During distribution work, preserve the standard license text and establish sufficient contributor rights for first-party hosting and any separate commercial permissions. No contributor agreement, copyright assignment, alternative public license or license-key system is selected by D221. Dependency/data licenses remain separately applicable; choosing an application license does not relicense upstream material.

## Projected dependency compatibility

Checked 17 October 2026. **No incompatibility has been identified in the selected direct components.** This table covers projected application components. The workspace now has library manifests and a lockfile; the concrete LoTW dependency assessment below supplements it. A complete packaged-runtime audit still depends on the application and distribution artifacts. Compatibility findings below are interpretations of the cited permissions, not an upstream certification of this combination.

| Component and selection status | License evidence | Assessment for Elastic License 2.0 |
| --- | --- | --- |
| egui, eframe and egui_extras; selected starting direction | Current [egui workspace](https://raw.githubusercontent.com/emilk/egui/main/Cargo.toml), [eframe package](https://raw.githubusercontent.com/emilk/egui/main/crates/eframe/Cargo.toml), and [egui_extras package](https://raw.githubusercontent.com/emilk/egui/main/crates/egui_extras/Cargo.toml): MIT OR Apache-2.0. MIT is the conventional license name, not a new project initialism. | Permissive options permit inclusion in a differently licensed application. Preserve applicable upstream copyright, license and notice material. No copyleft requirement to license Clacks under the toolkit's terms. |
| Serde; selected | [Serde package](https://raw.githubusercontent.com/serde-rs/serde/master/serde/Cargo.toml): MIT OR Apache-2.0. | Same permissive compatibility assessment. Exact serializer and additional crates remain unselected. |
| AccessKit accessibility bridge and wgpu renderer; projected egui integration components, not independently pinned | [AccessKit workspace](https://raw.githubusercontent.com/AccessKit/accesskit/main/Cargo.toml) and [wgpu workspace](https://raw.githubusercontent.com/gfx-rs/wgpu/trunk/Cargo.toml): MIT OR Apache-2.0. | No direct license obstacle identified. This does not audit every enabled backend, system library or shader/compiler redistributable. |
| SQLite core; selected client engine | [SQLite copyright statement](https://sqlite.org/copyright.html): public domain. | No application-license conflict identified. This does not establish the license of Rust bindings or extensions. |
| PostgreSQL; selected separate server database service | [PostgreSQL license](https://www.postgresql.org/about/licence/): permissive use/modification/distribution terms with notice obligations. | No application-license conflict identified. Database driver and container distribution are separate inventories. |
| TrustedQSL source; selected behavioral reference for a Rust port | Pinned [library license](trustedqsl/upstream/src/LICENSE), [root license](trustedqsl/upstream/LICENSE.txt), and file-level notices. | Main terms permit modified source/binary redistribution, require retained notices/disclaimers, restrict use of American Radio Relay League (ARRL) marks in product names and prohibit endorsement claims. No apparent hosting-policy conflict. A translated/adapted implementation must retain applicable upstream rights and notices; Rust translation does not erase them. |
| Rust OpenSSL bindings and OpenSSL; considered but rejected for LoTW under D224 | [Rust bindings](https://raw.githubusercontent.com/sfackler/rust-openssl/master/openssl/Cargo.toml): Apache-2.0. [OpenSSL licensing](https://openssl-library.org/source/license/): 3.0 and later use Apache-2.0; older releases use the older OpenSSL/SSLeay terms. | Modern OpenSSL presents no apparent license incompatibility. Exact bundled version/providers must be inventoried if selected. This is separate from older copied code embedded in TrustedQSL. |

### Concrete source-port obligation

The pinned [openssl_cert.cpp](trustedqsl/upstream/src/openssl_cert.cpp) states that portions were copied from OpenSSL application sources and carries the older OpenSSL and Eric Young/SSLeay license texts. Those texts add attribution, advertising-acknowledgment, naming and notice requirements; the SSLeay text also preserves its distribution terms on derivatives. They are not replaced by TrustedQSL's top-level license.

If the Rust port adapts those portions, carry forward the applicable terms and acknowledgments. Linking a modern Apache-licensed OpenSSL release does not retroactively relicense copied historical source. This is an identified compliance obligation, not an identified incompatibility with licensing Clacks's own code under Elastic License 2.0. Trace actual adapted portions during the source port rather than treating every protocol-compatible implementation as necessarily copying every upstream file.

### Copyleft, fonts, data and process boundaries

- **Strong copyleft:** the GNU General Public License and GNU Affero General Public License ordinarily cannot govern a linked/derived combined Clacks program while Elastic's additional hosting restrictions remain. A separate commercial permission or applicable linking exception can change that assessment. This is consistent with D214, not a newly selected dependency policy. See the [GPL version 3 license, including sections 5 and 10](https://opensource.org/license/gpl-3.0). Merely supplying source does not cure incompatible use restrictions.
- **Limited copyleft:** do not reject every copyleft license categorically. The [Mozilla Public License (MPL) 2.0](https://www.mozilla.org/en-US/MPL/2.0/FAQ/) permits larger works under other terms while preserving rights/source availability for covered files. The [GNU Lesser General Public License (LGPL) version 3](https://opensource.org/license/lgpl-3-0) permits combined works under other terms with conditions concerning library modification, notices, reverse engineering for debugging those modifications, and replacement/relinking. Exact version, integration and distribution matter; Rust static linking is not automatically forbidden, but does not remove those obligations. No LGPL component is required by the selected desktop toolkit itself.
- **Fonts:** egui embeds font assets with their own licensing histories, as shown in its [default-font source](https://raw.githubusercontent.com/emilk/egui/main/crates/epaint_default_fonts/src/lib.rs). For example, its bundled [SIL Open Font License text](https://raw.githubusercontent.com/emilk/egui/main/crates/epaint_default_fonts/fonts/OFL.txt) permits software bundling but keeps the fonts under their own terms. Inventory each actually bundled font rather than applying the crate's code license to all assets. No font-related application-license blocker has been identified; the exact asset inventory remains to be checked.
- **Geographic data:** [location research](location-lookup.md) has not cleared every proposed geographic dataset's provenance/redistribution rights. Public availability or a permissively licensed loader does not establish rights to underlying geometry. This is an existing unresolved data-distribution question, not an established conflict with Elastic License 2.0.
- **Separate software:** exchanging documented network messages with WS Suite or GridTracker does not itself incorporate their implementation into Clacks. Likewise a separate database or ordinary container utility is not automatically part of a single derived program. Redistribution of such software still carries its own obligations; process boundaries are not a universal workaround for copying or tightly integrated derived works.

Retain upstream license grants on third-party portions; scope Clacks's own license accordingly. The [MIT text](https://raw.githubusercontent.com/emilk/egui/main/LICENSE-MIT) and [Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0) allow this style of application distribution subject to their conditions. Do not claim that Clacks's hosting restriction revokes independent upstream permissions on extracted third-party libraries.

This review supports the Elastic License 2.0 selection under D221. When actual versions/features are selected, review the resolved dependency graph, native libraries, embedded assets and distribution packages. Drivers, networking/authentication, credential storage, cryptographic parsing and geographic data remain selection work; they are not approved by association with permissive direct dependencies. The projected application components still require their own resolved dependency and distribution checks; the library inventory below does not constitute final release-license clearance.

## Repository maintenance dependencies

The Rust-only repository maintenance command uses `roxmltree` 0.21.1 under
MIT OR Apache-2.0. It is confined to `clacks-reference-data`/`clacks-lotw-data` and is not a
dependency of the model, synchronization or signing libraries. Preserve its
applicable notices when distributing the maintenance tool.

The separate `clacks-lotw-fixtures` command enables `clacks-reference-data`'s
optional `fixtures` feature. It adds Rust-only construction dependencies including
`rand_chacha` 0.3.1, `num-bigint` 0.4.8, `sha2` 0.10.9/0.11.1, `hmac` 0.13.0,
`pkcs5` 0.8.1, `cbc` 0.2.1, `des` 0.9.0, `rc2` 0.9.0 and `hex` 0.4.3,
alongside the Rust cryptography families listed below. Their Cargo metadata
provides MIT/Apache-2.0 options. Keep their applicable notices if distributing
the fixture tool. This feature is not a dependency of application libraries.
The RSA advisory assessment below still applies; fixture keys are public test
inputs and cannot be used for real credentials.

## Contributor rights

The [actual agreement comparison](contributor-agreements.md) preserves findings on published forms. D234 leaves substantive contributor-agreement adaptation and legal drafting/review to a qualified lawyer. [Findings and questions for counsel](jetbrains-cla-review.md) accompany the existing hosting/licensing requirements. No contributor agreement or original-license-continuity policy is selected. Elastic's short policy notice is not a substitute for its unreviewed agreement text. D221 remains unchanged.

## Locked LoTW library dependencies

The `clacks-lotw` graph in the committed lockfile uses `p12-keystore` 0.4.1
with `pbes1`, `x509-parser` 0.18.1 without verification, `rsa` 0.9.10,
`sha1` 0.10.7 with `oid`, `base64` 0.22.1 and `rand` 0.8.8. Archive decoding
also resolves `cms` 0.3.0-pre.1, `pkcs12` 0.2.0-pre.0, `der` 0.8.2,
`pkcs8` 0.11.0, `digest` 0.11.3 and `sha1` 0.11.0. RSA retains `der` 0.7.10,
`pkcs8` 0.10.2 and `digest` 0.10.7. These families exchange encoded bytes;
the prereleases and parallel families are intentional maintenance costs.

Cargo metadata for third-party packages in the resolved LoTW graph (including
test and target-specific packages) declares permissive MIT, Apache-2.0, BSD, Unicode-3.0, Unlicense or Zlib
options. `r-efi` offers MIT/Apache-2.0 alternatives to LGPL; use a permissive
option. No linked OpenSSL, TQSL, C++ or other native cryptographic implementation
is present. Operating-system randomness uses platform APIs. Retain each selected
license and applicable notices when packaging; metadata inspection is not a
complete binary/asset distribution audit. Existing TrustedQSL and historical
OpenSSL/SSLeay notices remain in the reference tree and must not be discarded.
The Rust implementation follows protocol behavior rather than copying the
historical OpenSSL application routines.
