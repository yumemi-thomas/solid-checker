// Policy-1 proof-file issuance was removed by the Phase 19 atomic receipt cut.
// The command name remains only to give existing automation a deterministic
// migration refusal; audit transcripts are not replayable acceptance authority.

import process from "node:process";

export const contractVerifyHelp = `Usage:
  solid-checker contract verify

This command no longer issues accepted contracts from caller-authored proof
files. Use "solid-checker contract certify" for policy-2 reacquisition and
certification.
`;

export function parseVerifyArguments(arguments_) {
  const help = arguments_.some(argument => ["--help", "-h"].includes(argument));
  if (help) return { help: true };
  if (arguments_.length > 0) {
    throw new Error(
      "contract verify proof-file issuance was retired; use contract certify"
    );
  }
  return { help: false };
}

export async function verifyContract(arguments_) {
  const options = parseVerifyArguments(arguments_);
  if (options.help) {
    process.stdout.write(contractVerifyHelp);
    return;
  }
  throw new Error(
    "contract verify proof-file issuance was retired; use contract certify"
  );
}
