// The lines a finding of the checker's JSON report holds, for the judges that read a site's verdict from the findings on its line (scripts/fuzz/checker.mjs `siteVerdicts`,
// scripts/runtime-oracle.mjs).
//
// The report locates a finding by its bytes and by the line and column where it starts; it names no end line, so a finding holds its start line (a judge reads `endLine` when
// a report has one). One finding is about its file as a whole: an uncertifiable result of subject `file` (ADR 1152's notice that a program writes a name with an escape, which
// says nothing in the file is proven), whose span is the file. It holds every line of the file, so a site whose finding a misread name hid still stands under it.

/** Whether a finding is an obligation about its file as a whole. */
export const holdsWholeFile = (finding) => finding.kind === "uncertifiable" && finding.subjectKind === "file";

/** The last line a finding holds: its own end line when the report names one, else its start line, and for a file-wide obligation every line of the file. */
export const lastLine = (finding) => (holdsWholeFile(finding) ? Number.MAX_SAFE_INTEGER : finding.primaryLocation.endLine ?? finding.primaryLocation.line);
