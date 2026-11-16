#include "VocalTractLabBackend/Speaker.h"
#include "VocalTractLabBackend/VocalTract.h"
#include "VocalTractLabBackend/GesturalScore.h"
#include "VocalTractLabBackend/Glottis.h"
#include <cstdio>
#include <string>
static void add(GestureSequence& seq, Gesture g) { seq.appendGesture(g); }
static Gesture mk(double dur, const std::string& s, bool neutral, double d = 0.0, double tau = 0.012, double slope = 0.0) {
  Gesture g; g.duration_s = dur; g.sVal = s; g.neutral = neutral; g.dVal = d; g.tau_s = tau; g.slope = slope; return g;
}
int main(int argc, char** argv) {
  Speaker* spk = new Speaker(std::string(argv[1]));
  VocalTract* vt = spk->getVocalTract();
  Glottis* gl = spk->getGlottisModels()[spk->getSelectedGlottis()];
  GesturalScore* gs = new GesturalScore(vt, gl);
  for (int i = 0; i < GesturalScore::NUM_GESTURE_TYPES; i++) gs->gestures[i].clear();
  auto& G = gs->gestures;
  add(G[GesturalScore::VOWEL_GESTURE], mk(0.20, "a", false));
  add(G[GesturalScore::VOWEL_GESTURE], mk(0.40, "i", false, 0, 0.016));
  add(G[GesturalScore::TONGUE_BODY_GESTURE], mk(0.16, "", true, 0, 0.005));
  add(G[GesturalScore::TONGUE_BODY_GESTURE], mk(0.08, "tb-velar-closure", false, 0, 0.012));
  add(G[GesturalScore::TONGUE_BODY_GESTURE], mk(0.36, "", true, 0, 0.005));
  add(G[GesturalScore::LIP_GESTURE], mk(0.30, "", true, 0, 0.005));
  add(G[GesturalScore::LIP_GESTURE], mk(0.06, "ll-labial-closure", false, 0, 0.010));
  add(G[GesturalScore::LIP_GESTURE], mk(0.24, "", true, 0, 0.005));
  add(G[GesturalScore::TONGUE_TIP_GESTURE], mk(0.40, "", true, 0, 0.005));
  add(G[GesturalScore::TONGUE_TIP_GESTURE], mk(0.06, "tt-alveolar-fricative", false, 0, 0.014));
  add(G[GesturalScore::TONGUE_TIP_GESTURE], mk(0.14, "", true, 0, 0.005));
  add(G[GesturalScore::VELIC_GESTURE], mk(0.30, "", true, -0.1, 0.01));
  add(G[GesturalScore::VELIC_GESTURE], mk(0.10, "", false, 0.5, 0.01));
  add(G[GesturalScore::VELIC_GESTURE], mk(0.20, "", true, -0.1, 0.01));
  add(G[GesturalScore::GLOTTAL_SHAPE_GESTURE], mk(0.15, "modal", false, 0, 0.012));
  add(G[GesturalScore::GLOTTAL_SHAPE_GESTURE], mk(0.10, "voiceless-plosive", false, 0, 0.012));
  add(G[GesturalScore::GLOTTAL_SHAPE_GESTURE], mk(0.35, "modal", false, 0, 0.012));
  add(G[GesturalScore::F0_GESTURE], mk(0.01, "", false, 80.0, 0.03));
  add(G[GesturalScore::F0_GESTURE], mk(0.30, "", false, 83.0, 0.03, 1.0));
  add(G[GesturalScore::F0_GESTURE], mk(0.29, "", false, 78.0, 0.03));
  add(G[GesturalScore::PRESSURE_GESTURE], mk(0.01, "", false, 0.0, 0.005));
  add(G[GesturalScore::PRESSURE_GESTURE], mk(0.50, "", false, 8000.0, 0.005));
  add(G[GesturalScore::PRESSURE_GESTURE], mk(0.09, "", false, 0.0, 0.005));
  gs->calcCurves();
  int ng = (int)gl->controlParam.size();
  printf("NUMGLOTTIS %d\n", ng);
  for (double t = 0.0; t <= 0.6001; t += 0.0125) {
    double vp[VocalTract::NUM_PARAMS], gp[32];
    gs->getParams(t, vp, gp);
    printf("T %.4f", t);
    for (int i = 0; i < VocalTract::NUM_PARAMS; i++) printf(" %.9f", vp[i]);
    printf(" |");
    for (int i = 0; i < ng; i++) printf(" %.9f", gp[i]);
    printf("\n");
  }
  return 0;
}
