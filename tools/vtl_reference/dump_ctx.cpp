#include "VocalTractLabBackend/VocalTract.h"
#include "VocalTractLabBackend/GesturalScore.h"
#include <cstdio>
#include <string>
int main(int argc, char** argv) {
  VocalTract& vt = *new VocalTract();
  vt.readFromXml(argv[1]);
  const char* consonants[] = {"ll-labial-closure","tt-alveolar-closure","tt-postalveolar-closure","tb-velar-closure","tt-alveolar-fricative","tt-postalveolar-fricative","tb-palatal-fricative"};
  const char* vowels[] = {"a","e","i","o","u","E:","@","2:","y"};
  // Vowel subspace coordinates of every vowel.
  for (const char* v : vowels) {
    int idx = vt.getShapeIndex(v);
    if (idx < 0) continue;
    double at, bt, al, bl;
    GesturalScore::mapToVowelSubspace(&vt, vt.shapes[idx].param, at, bt, al, bl);
    printf("COORD %s %.9f %.9f %.9f %.9f\n", v, at, bt, al, bl);
    GesturalScore::limitVowelSubspaceCoord(at, bt, al, bl);
    printf("LIMITED %s %.9f %.9f %.9f %.9f\n", v, at, bt, al, bl);
  }
  for (const char* c : consonants) for (const char* v : vowels) {
    if (vt.getShapeIndex(v) < 0) continue;
    double p[VocalTract::NUM_PARAMS];
    bool ok = GesturalScore::getContextDependentConsonant(&vt, c, v, p);
    printf("CTX %s %s %d", c, v, ok ? 1 : 0);
    for (int i = 0; i < VocalTract::NUM_PARAMS; i++) printf(" %.9f", p[i]);
    printf("\n");
  }
  return 0;
}
