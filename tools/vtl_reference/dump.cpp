#include "VocalTractLabBackend/VocalTract.h"
#include "VocalTractLabBackend/Tube.h"
#include <cstdio>
#include <string>
int main(int argc, char** argv) {
  VocalTract& vt = *new VocalTract();
  vt.readFromXml(argv[1]);
  Tube& tube = *new Tube();
  for (size_t s = 0; s < vt.shapes.size(); s++) {
    const auto& sh = vt.shapes[s];
    for (int i = 0; i < VocalTract::NUM_PARAMS; i++) vt.param[i].x = sh.param[i];
    vt.calculateAll();
    vt.getTube(&tube);
    printf("SHAPE %s\n", sh.name.c_str());
    printf("META incisor %.9f tipElev %.9f\n", tube.teethPosition_cm, tube.tongueTipSideElevation);
    for (int i = Tube::FIRST_PHARYNX_SECTION; i <= Tube::LAST_PHARYNX_SECTION + Tube::NUM_MOUTH_SECTIONS; i++) {
      const auto& t = tube.pharynxMouthSection[i - Tube::FIRST_PHARYNX_SECTION];
      printf("S %d %.9f %.9f %d\n", i, t.area_cm2, t.length_cm, (int)t.articulator);
    }
    for (int i = 0; i < Tube::NUM_NOSE_SECTIONS; i++) {
      const auto& t = tube.noseSection[i];
      printf("N %d %.9f %.9f\n", i, t.area_cm2, t.length_cm);
    }
  }
  return 0;
}
