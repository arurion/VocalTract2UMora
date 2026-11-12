#include "VocalTractLabBackend/VocalTract.h"
#include <cstdio>
#include <string>
int main(int argc, char** argv) {
  VocalTract& vt = *new VocalTract();
  vt.readFromXml(argv[1]);
  std::string shapeName = argc > 2 ? argv[2] : "a";
  for (size_t s = 0; s < vt.shapes.size(); s++) {
    if (vt.shapes[s].name != shapeName) continue;
    for (int i = 0; i < VocalTract::NUM_PARAMS; i++) vt.param[i].x = vt.shapes[s].param[i];
  }
  vt.calculateAll();
  for (int k = 0; k < VocalTract::NUM_SURFACES; k++) {
    Surface& sf = vt.surface[k];
    printf("SURF %d %d %d\n", k, sf.numRibs, sf.numRibPoints);
    for (int i = 0; i < sf.numRibs; i++)
      for (int j = 0; j < sf.numRibPoints; j++) {
        Point3D p = sf.getVertex(i, j);
        printf("V %d %d %.9f %.9f %.9f\n", i, j, p.x, p.y, p.z);
      }
  }
  return 0;
}
